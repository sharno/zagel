use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::marker::PhantomData;

use iced::widget::pane_grid;
use iced::{Task, clipboard};

use crate::model::{Method, RequestDraft, RequestId, ResponsePreview};
use crate::net::send_request;
use crate::parser::{persist_request, write_http_file};
use crate::pathing::{GlobalEnvRoot, ProjectRoot};

use super::domain::{
    AddRequestPlan, GlobalEnvChangeOutcome, ProjectChangeOutcome, SavePlan,
};
use super::options::{RequestMode, apply_auth_headers, build_graphql_body};
use super::status::status_with_missing;
use super::{EditState, EditTarget, HeaderRow, Message, Zagel};

const MIN_SPLIT_RATIO: f32 = 0.2;
const FILE_SCAN_DEBOUNCE: Duration = Duration::from_millis(300);

struct Unplanned;
struct Planned;

struct SaveFlow<State> {
    plan: SavePlan,
    marker: PhantomData<State>,
}

impl SaveFlow<Unplanned> {
    fn from_app(app: &Zagel) -> Result<Self, String> {
        app.build_save_plan().map(|plan| Self {
            plan,
            marker: PhantomData,
        }).map_err(|err| err.to_string())
    }

    fn into_planned(self) -> SaveFlow<Planned> {
        SaveFlow {
            plan: self.plan,
            marker: PhantomData,
        }
    }
}

impl SaveFlow<Planned> {
    fn into_task(self) -> Task<Message> {
        let (root, selection, draft, explicit_path) = self.plan.into_persist_args();
        Task::perform(
            async move {
                persist_request(root, selection, draft, explicit_path)
                    .await
                    .map_err(|e| e.to_string())
            },
            Message::Saved,
        )
    }
}

struct AddRequestFlow<State> {
    plan: AddRequestPlan,
    marker: PhantomData<State>,
}

impl AddRequestFlow<Unplanned> {
    fn from_app(app: &Zagel) -> Result<Self, String> {
        app.build_add_request_plan().map(|plan| Self {
            plan,
            marker: PhantomData,
        }).map_err(|err| err.to_string())
    }

    fn into_planned(self) -> AddRequestFlow<Planned> {
        AddRequestFlow {
            plan: self.plan,
            marker: PhantomData,
        }
    }
}

impl AddRequestFlow<Planned> {
    fn into_parts(self) -> (PathBuf, RequestDraft, PathBuf) {
        (
            self.plan.file_path,
            self.plan.new_draft,
            self.plan.project_root,
        )
    }
}

fn clamp_ratio(ratio: f32) -> f32 {
    ratio.clamp(MIN_SPLIT_RATIO, 1.0 - MIN_SPLIT_RATIO)
}

const fn edit_selection_mut(edit_state: &mut EditState) -> Option<&mut HashSet<EditTarget>> {
    match edit_state {
        EditState::On { selection } => Some(selection),
        EditState::Off => None,
    }
}

fn remap_edit_selection(
    edit_state: &mut EditState,
    mut map: impl FnMut(EditTarget) -> EditTarget,
) {
    let Some(selection) = edit_selection_mut(edit_state) else {
        return;
    };
    let mut next = HashSet::with_capacity(selection.len());
    for item in selection.drain() {
        next.insert(map(item));
    }
    *selection = next;
}

fn swap_request_indices_in_selection_http(
    selection: &mut Option<RequestId>,
    path: &PathBuf,
    a: usize,
    b: usize,
) {
    if let Some(RequestId::HttpFile {
        path: sel_path,
        index,
    }) = selection.as_mut()
        && sel_path == path
    {
        if *index == a {
            *index = b;
        } else if *index == b {
            *index = a;
        }
    }
}

fn swap_request_indices_in_edit_selection_http(
    edit_state: &mut EditState,
    path: &PathBuf,
    a: usize,
    b: usize,
) {
    remap_edit_selection(edit_state, |item| match item {
        EditTarget::Request(RequestId::HttpFile { path: p, index }) => {
            if p == *path {
                if index == a {
                    EditTarget::Request(RequestId::HttpFile { path: p, index: b })
                } else if index == b {
                    EditTarget::Request(RequestId::HttpFile { path: p, index: a })
                } else {
                    EditTarget::Request(RequestId::HttpFile { path: p, index })
                }
            } else {
                EditTarget::Request(RequestId::HttpFile { path: p, index })
            }
        }
        other => other,
    });
}

fn remove_edit_targets_for_root(edit_state: &mut EditState, root: &ProjectRoot) {
    if let EditState::On { selection } = edit_state {
        selection.retain(|target| match target {
            EditTarget::Collection(path)
            | EditTarget::Request(RequestId::HttpFile { path, .. }) => {
                !path.starts_with(root.as_path())
            }
        });
    }
}

#[allow(clippy::too_many_lines)]
impl Zagel {
    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FilesChanged => {
                if !self.should_scan() {
                    return Task::none();
                }
                if matches!(self.edit_state, EditState::On { .. }) {
                    self.pending_rescan = true;
                    return Task::none();
                }
                let now = Instant::now();
                if let Some(last) = self.last_scan
                    && now.duration_since(last) < FILE_SCAN_DEBOUNCE
                {
                    return Task::none();
                }
                self.last_scan = Some(now);
                self.pending_rescan = false;
                self.rescan_files()
            }
            Message::WatcherUnavailable(message) => {
                self.update_status_with_missing(&message);
                Task::none()
            }
            Message::HttpFilesLoaded(files) => {
                if !self.should_scan() {
                    return Task::none();
                }
                let mut workspace = self.workspace.ensured_configured_state();
                workspace.replace_http_files(files);
                let loaded_paths = workspace
                    .http_files()
                    .keys()
                    .cloned()
                    .collect::<HashSet<PathBuf>>();
                workspace
                    .http_file_order_mut()
                    .retain(|path| loaded_paths.contains(path));
                let mut new_paths: Vec<PathBuf> = workspace
                    .http_files()
                    .keys()
                    .filter(|path| !workspace.http_file_order().contains(path))
                    .cloned()
                    .collect();
                new_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
                workspace.http_file_order_mut().extend(new_paths);
                if let Some(RequestId::HttpFile { path, index }) = workspace.selection_cloned()
                    && workspace
                        .http_files()
                        .get(&path)
                        .is_none_or(|file| index >= file.requests.len())
                {
                    workspace.set_selection(None);
                }
                self.refresh_visible_environments();
                Task::none()
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, clamp_ratio(ratio));
                Task::none()
            }
            Message::WorkspacePaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.workspace_panes.resize(split, clamp_ratio(ratio));
                Task::none()
            }
            Message::BuilderPaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.builder_panes.resize(split, clamp_ratio(ratio));
                Task::none()
            }
            Message::ToggleCollection(path) => {
                if !self.collapsed_collections.remove(&path) {
                    self.collapsed_collections.insert(path);
                }
                Task::none()
            }
            Message::ToggleEditMode => {
                let was_editing = matches!(self.edit_state, EditState::On { .. });
                self.edit_state = if was_editing {
                    EditState::Off
                } else {
                    EditState::On {
                        selection: HashSet::new(),
                    }
                };
                if was_editing {
                    self.persist_state();
                    if self.pending_rescan {
                        self.pending_rescan = false;
                        self.last_scan = Some(Instant::now());
                        return self.rescan_files();
                    }
                }
                Task::none()
            }
            Message::ToggleEditSelection(target) => {
                if let Some(selection) = edit_selection_mut(&mut self.edit_state)
                    && !selection.remove(&target)
                {
                    selection.insert(target);
                }
                Task::none()
            }
            Message::DeleteSelected => {
                let edit_selection = match &self.edit_state {
                    EditState::On { selection } if !selection.is_empty() => selection.clone(),
                    _ => return Task::none(),
                };

                let errors = {
                    let Some(mut workspace) = self.workspace.configured_state() else {
                        return Task::none();
                    };

                    let mut remove_file_paths = Vec::new();
                    let mut request_ids = Vec::new();

                    for target in &edit_selection {
                        match target {
                            EditTarget::Collection(path) => {
                                remove_file_paths.push(path.clone());
                            }
                            EditTarget::Request(id) => request_ids.push(id.clone()),
                        }
                    }

                    remove_file_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
                    remove_file_paths.dedup();

                    let remove_files_set: HashSet<PathBuf> = remove_file_paths.iter().cloned().collect();

                    let mut file_request_removals = std::collections::HashMap::new();
                    for id in request_ids {
                        let RequestId::HttpFile { path, index } = id;
                        if remove_files_set.contains(&path) {
                            continue;
                        }
                        file_request_removals
                            .entry(path)
                            .or_insert_with(Vec::new)
                            .push(index);
                    }

                    let mut errors = Vec::new();

                    for (path, mut indices) in file_request_removals {
                        if let Some(file) = workspace.http_files_mut().get_mut(&path) {
                            let mut updated_requests = file.requests.clone();
                            indices.sort_unstable();
                            indices.dedup();
                            for idx in indices.into_iter().rev() {
                                if idx < updated_requests.len() {
                                    updated_requests.remove(idx);
                                }
                            }
                            if let Err(err) = write_http_file(&file.path, &updated_requests) {
                                errors.push(format!(
                                    "Failed to update {}: {}",
                                    file.path.display(),
                                    err
                                ));
                            } else {
                                file.requests = updated_requests;
                            }
                        }
                    }

                    for path in &remove_file_paths {
                        match fs::remove_file(path) {
                            Ok(()) => {}
                            Err(_err) if !path.exists() => {}
                            Err(err) => {
                                errors.push(format!("Failed to delete {}: {}", path.display(), err));
                            }
                        }
                        workspace.http_files_mut().remove(path);
                    }
                    if !remove_file_paths.is_empty() {
                        workspace
                            .http_file_order_mut()
                            .retain(|path| !remove_files_set.contains(path));
                    }

                    if let Some(RequestId::HttpFile { path, index }) = workspace.selection_cloned()
                        && workspace
                            .http_files()
                            .get(&path)
                            .is_none_or(|file| index >= file.requests.len())
                    {
                        workspace.set_selection(None);
                    }

                    errors
                };

                if let EditState::On { selection } = &mut self.edit_state {
                    selection.clear();
                }
                if errors.is_empty() {
                    self.update_status_with_missing("Deleted selection");
                } else {
                    self.update_status_with_missing(&errors.join("; "));
                }

                self.refresh_visible_environments();

                Task::none()
            }
            Message::MoveCollectionUp(path) => {
                if let Some(mut workspace) = self.workspace.configured_state()
                    && let Some(pos) = workspace.http_file_order().iter().position(|p| p == &path)
                    && pos > 0
                {
                    workspace.http_file_order_mut().swap(pos, pos - 1);
                }
                Task::none()
            }
            Message::MoveCollectionDown(path) => {
                if let Some(mut workspace) = self.workspace.configured_state()
                    && let Some(pos) = workspace.http_file_order().iter().position(|p| p == &path)
                    && pos + 1 < workspace.http_file_order().len()
                {
                    workspace.http_file_order_mut().swap(pos, pos + 1);
                }
                Task::none()
            }
            Message::MoveRequestUp(id) => {
                let RequestId::HttpFile { path, index } = &id;
                if *index == 0 {
                    return Task::none();
                }
                let Some(mut workspace) = self.workspace.configured_state() else {
                    return Task::none();
                };
                let mut new_index = None;
                let mut status_error = None;
                if let Some(file) = workspace.http_files_mut().get_mut(path)
                    && *index < file.requests.len()
                {
                    let updated_index = *index - 1;
                    file.requests.swap(*index, updated_index);
                    if let Err(err) = write_http_file(&file.path, &file.requests) {
                        file.requests.swap(*index, updated_index);
                        status_error =
                            Some(format!("Failed to reorder {}: {}", file.path.display(), err));
                    } else {
                        new_index = Some(updated_index);
                    }
                }
                if let Some(updated_index) = new_index {
                    swap_request_indices_in_selection_http(
                        workspace.selection_mut(),
                        path,
                        *index,
                        updated_index,
                    );
                    swap_request_indices_in_edit_selection_http(
                        &mut self.edit_state,
                        path,
                        *index,
                        updated_index,
                    );
                }
                if let Some(message) = status_error {
                    self.update_status_with_missing(&message);
                }
                Task::none()
            }
            Message::MoveRequestDown(id) => {
                let RequestId::HttpFile { path, index } = &id;
                let Some(mut workspace) = self.workspace.configured_state() else {
                    return Task::none();
                };
                let mut new_index = None;
                let mut status_error = None;
                if let Some(file) = workspace.http_files_mut().get_mut(path)
                    && *index + 1 < file.requests.len()
                {
                    let updated_index = *index + 1;
                    file.requests.swap(*index, updated_index);
                    if let Err(err) = write_http_file(&file.path, &file.requests) {
                        file.requests.swap(*index, updated_index);
                        status_error =
                            Some(format!("Failed to reorder {}: {}", file.path.display(), err));
                    } else {
                        new_index = Some(updated_index);
                    }
                }
                if let Some(updated_index) = new_index {
                    swap_request_indices_in_selection_http(
                        workspace.selection_mut(),
                        path,
                        *index,
                        updated_index,
                    );
                    swap_request_indices_in_edit_selection_http(
                        &mut self.edit_state,
                        path,
                        *index,
                        updated_index,
                    );
                }
                if let Some(message) = status_error {
                    self.update_status_with_missing(&message);
                }
                Task::none()
            }
            Message::EnvironmentsLoaded(envs) => {
                self.workspace.set_all_environments(envs);
                self.refresh_visible_environments();
                self.persist_state();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::Select(id) => {
                self.apply_selection(&id);
                Task::none()
            }
            Message::MethodSelected(method) => {
                self.draft.method = method;
                Task::none()
            }
            Message::UrlChanged(url) => {
                self.draft.url = url;
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::TitleChanged(title) => {
                self.draft.title = title;
                Task::none()
            }
            Message::ModeChanged(mode) => {
                self.mode = mode;
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::BodyEdited(action) => {
                self.body_editor.perform(action);
                self.draft.body = self.body_editor.text();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::GraphqlQueryEdited(action) => {
                self.graphql_query.perform(action);
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::GraphqlVariablesEdited(action) => {
                self.graphql_variables.perform(action);
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::AuthChanged(new_auth) => {
                self.auth = new_auth;
                Task::none()
            }
            Message::HeaderNameChanged(idx, value) => {
                if let Some(row) = self.header_rows.get_mut(idx) {
                    row.name = value;
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderValueChanged(idx, value) => {
                if let Some(row) = self.header_rows.get_mut(idx) {
                    row.value = value;
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderAdded => {
                self.header_rows.push(HeaderRow {
                    name: String::new(),
                    value: String::new(),
                });
                self.rebuild_headers_from_rows();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderRemoved(idx) => {
                if idx < self.header_rows.len() {
                    self.header_rows.remove(idx);
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::ResponseViewChanged(display) => {
                self.response_display = display;
                self.update_response_viewer();
                Task::none()
            }
            Message::ResponseTabChanged(tab) => {
                self.response_tab = tab;
                Task::none()
            }
            Message::ToggleShortcutsHelp => {
                self.show_shortcuts = !self.show_shortcuts;
                Task::none()
            }
            Message::CopyResponseRaw => {
                let text = self.response.as_ref().map_or_else(
                    || self.response_viewer.text(),
                    |response| response.body.raw().to_string(),
                );
                clipboard::write(text).map(|()| Message::CopyComplete)
            }
            Message::CopyResponsePretty => {
                let Some(text) = self
                    .response
                    .as_ref()
                    .and_then(|response| response.body.pretty_text())
                else {
                    return Task::none();
                };
                clipboard::write(text.to_string()).map(|()| Message::CopyComplete)
            }
            Message::CopyComplete => Task::none(),
            Message::AddRequest => {
                let planned = match AddRequestFlow::<Unplanned>::from_app(self) {
                    Ok(flow) => flow.into_planned(),
                    Err(err) => {
                        self.update_status_with_missing(&err);
                        return Task::none();
                    }
                };

                let (path, draft, project_root) = planned.into_parts();
                let mut workspace = self
                    .workspace
                    .configured_state()
                    .expect("add-request flow requires configured workspace");
                let file = workspace
                    .http_files_mut()
                    .get_mut(&path)
                    .expect("add-request flow requires selected file to be loaded");

                file.requests.push(draft.clone());
                let new_idx = file.requests.len() - 1;
                workspace.set_selection(Some(RequestId::HttpFile {
                    path: path.clone(),
                    index: new_idx,
                }));
                self.update_status_with_missing("Saving new request...");

                Task::perform(
                    async move {
                        persist_request(project_root, None, draft, Some(path))
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::Saved,
                )
            }
            Message::Send => {
                let env = self.environments.get(self.active_environment).cloned();
                let mut draft = self.draft.clone();
                let mut extra_inputs: Vec<String> = Vec::new();
                if self.mode == RequestMode::GraphQl {
                    draft.method = Method::Post;
                    let query = self.graphql_query.text();
                    let variables = self.graphql_variables.text();
                    extra_inputs.push(query.clone());
                    extra_inputs.push(variables.clone());
                    draft.body = build_graphql_body(&query, &variables);
                    if !draft.headers.contains("Content-Type") {
                        draft.headers.push_str("\nContent-Type: application/json");
                    }
                }
                draft.headers = apply_auth_headers(&draft.headers, &self.auth);
                let extra_refs: Vec<&str> = extra_inputs.iter().map(String::as_str).collect();
                self.status_line = status_with_missing("Sending...", &draft, env.as_ref(), &extra_refs);
                Task::perform(
                    send_request(self.client.clone(), draft, env),
                    Message::ResponseReady,
                )
            }
            Message::ResponseReady(result) => {
                match result {
                    Ok(resp) => {
                        self.update_status_with_missing("Received response");
                        self.response = Some(crate::app::view::ResponseData::from_preview(resp));
                    }
                    Err(err) => {
                        self.update_status_with_missing("Request failed");
                        self.response =
                            Some(crate::app::view::ResponseData::from_preview(ResponsePreview::error(err)));
                    }
                }
                self.update_response_viewer();
                Task::none()
            }
            Message::EnvironmentChanged(name) => {
                if let Some((idx, _)) = self
                    .environments
                    .iter()
                    .enumerate()
                    .find(|(_, env)| env.name == name)
                {
                    self.active_environment = idx;
                    self.state.active_environment = Some(name);
                    self.persist_state();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::Save => {
                let planned = match SaveFlow::<Unplanned>::from_app(self) {
                    Ok(flow) => flow.into_planned(),
                    Err(err) => {
                        self.update_status_with_missing(&err);
                        return Task::none();
                    }
                };
                self.update_status_with_missing("Saving...");
                planned.into_task()
            }
            Message::Saved(result) => match result {
                Ok((path, index)) => {
                    self.workspace.set_selection(Some(RequestId::HttpFile {
                        path: path.clone(),
                        index,
                    }));
                    self.refresh_visible_environments();
                    self.update_status_with_missing(&format!("Saved to {}", path.display()));
                    Task::batch([Task::none(), self.rescan_files()])
                }
                Err(err) => {
                    self.update_status_with_missing(&format!("Save failed: {err}"));
                    Task::none()
                }
            },
            Message::SavePathChanged(path) => {
                self.save_path = path;
                Task::none()
            }
            Message::ProjectPathInputChanged(path) => {
                self.project_path_input = path;
                Task::none()
            }
            Message::AddProject => match ProjectRoot::parse_user_input(&self.project_path_input) {
                Ok(root) => match self.configuration.add_project(root) {
                    Ok(outcome) => {
                        self.project_path_input.clear();
                        self.workspace.sync_with_configuration(&self.configuration);
                        self.persist_state();
                        self.update_status_with_missing(outcome.status_message());
                        self.last_scan = Some(Instant::now());
                        self.rescan_files()
                    }
                    Err(err) => {
                        self.update_status_with_missing(&err.to_string());
                        Task::none()
                    }
                },
                Err(err) => {
                    self.update_status_with_missing(&err.to_string());
                    Task::none()
                }
            },
            Message::RemoveProject(root) => {
                if let Some(RequestId::HttpFile { path, .. }) = self.workspace.selection()
                    && path.starts_with(root.as_path())
                {
                    self.workspace.clear_selection();
                }
                remove_edit_targets_for_root(&mut self.edit_state, &root);

                let Ok(outcome) = self.configuration.remove_project(&root) else {
                    return Task::none();
                };
                self.workspace.sync_with_configuration(&self.configuration);
                self.refresh_visible_environments();
                self.persist_state();

                match outcome {
                    ProjectChangeOutcome::AddedAndScan => Task::none(),
                    ProjectChangeOutcome::RemovedAndScan => {
                        self.update_status_with_missing(outcome.status_message());
                        self.last_scan = Some(Instant::now());
                        self.rescan_files()
                    }
                    ProjectChangeOutcome::RemovedLastProject => {
                        self.workspace.clear_scan_cache();
                        self.refresh_visible_environments();
                        self.update_status_with_missing(outcome.status_message());
                        Task::none()
                    }
                }
            }
            Message::GlobalEnvPathInputChanged(path) => {
                self.global_env_path_input = path;
                Task::none()
            }
            Message::AddGlobalEnvRoot => match GlobalEnvRoot::parse_user_input(&self.global_env_path_input) {
                Ok(root) => match self.configuration.add_global_env(root) {
                    Ok(outcome) => {
                        self.global_env_path_input.clear();
                        self.workspace.sync_with_configuration(&self.configuration);
                        self.persist_state();
                        self.update_status_with_missing(outcome.status_message());
                        match outcome {
                            GlobalEnvChangeOutcome::AddedRescan => {
                                self.last_scan = Some(Instant::now());
                                self.rescan_files()
                            }
                            GlobalEnvChangeOutcome::AddedIdle
                            | GlobalEnvChangeOutcome::RemovedRescan
                            | GlobalEnvChangeOutcome::RemovedIdle => Task::none(),
                        }
                    }
                    Err(err) => {
                        self.update_status_with_missing(&err.to_string());
                        Task::none()
                    }
                },
                Err(err) => {
                    self.update_status_with_missing(&err.to_string());
                    Task::none()
                }
            },
            Message::RemoveGlobalEnvRoot(root) => {
                let Ok(outcome) = self.configuration.remove_global_env(&root) else {
                    return Task::none();
                };
                self.workspace.sync_with_configuration(&self.configuration);
                if !self.should_scan() {
                    self.workspace.clear_scan_cache();
                }
                self.persist_state();
                self.refresh_visible_environments();
                self.update_status_with_missing(outcome.status_message());
                match outcome {
                    GlobalEnvChangeOutcome::RemovedRescan => {
                        self.last_scan = Some(Instant::now());
                        self.rescan_files()
                    }
                    GlobalEnvChangeOutcome::AddedRescan
                    | GlobalEnvChangeOutcome::AddedIdle
                    | GlobalEnvChangeOutcome::RemovedIdle => Task::none(),
                }
            }
        }
    }
}
