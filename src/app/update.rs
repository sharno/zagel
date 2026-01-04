use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::pane_grid;
use iced::{Task, clipboard};

use crate::model::{Method, RequestDraft, RequestId, ResponsePreview};
use crate::net::send_request;
use crate::parser::{persist_request, write_http_file};

use super::history::Focus;
use super::options::{RequestMode, apply_auth_headers, build_graphql_body};
use super::reducer::{Action, Effect, HeaderIndex, reduce};
use super::state::{AppModel, EditState, SplitRatio, ViewState};
use super::{EditTarget, Message, Zagel};

const FILE_SCAN_DEBOUNCE: Duration = Duration::from_millis(300);

const fn edit_selection_mut(
    edit_state: &mut EditState,
) -> Option<&mut HashSet<EditTarget>> {
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
    if let Some(RequestId::HttpFile { path: sel_path, index }) = selection.as_mut()
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

impl Zagel {
    #[allow(clippy::too_many_lines)]
    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::FilesChanged => {
                if matches!(self.view.edit_state, EditState::On { .. }) {
                    self.view.pending_rescan = true;
                    return Task::none();
                }
                let now = Instant::now();
                if let Some(last) = self.view.last_scan
                    && now.duration_since(last) < FILE_SCAN_DEBOUNCE
                {
                    return Task::none();
                }
                self.view.last_scan = Some(now);
                self.view.pending_rescan = false;
                self.rescan_files()
            }
            Message::WatcherUnavailable(message) => {
                self.view.update_status_with_model(&message, &self.model);
                Task::none()
            }
            Message::HttpFilesLoaded(files) => {
                self.view.http_files = files;
                self.view
                    .http_file_order
                    .retain(|path| self.view.http_files.contains_key(path));
                let mut new_paths: Vec<PathBuf> = self
                    .view
                    .http_files
                    .keys()
                    .filter(|path| !self.view.http_file_order.contains(path))
                    .cloned()
                    .collect();
                new_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
                self.view.http_file_order.extend(new_paths);

                if let Some(RequestId::HttpFile { path, index }) = self.view.selection.clone()
                    && self
                        .view
                        .http_files
                        .get(&path)
                        .is_none_or(|file| index >= file.requests.len())
                {
                    self.view.selection = None;
                }

                Task::none()
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                let ratio = SplitRatio::new(ratio);
                self.view.panes.resize(split, ratio.get());
                Task::none()
            }
            Message::WorkspacePaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                let ratio = SplitRatio::new(ratio);
                self.view.workspace_panes.resize(split, ratio.get());
                Task::none()
            }
            Message::BuilderPaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                let ratio = SplitRatio::new(ratio);
                self.view.builder_panes.resize(split, ratio.get());
                Task::none()
            }
            Message::ToggleCollection(path) => {
                if !self.view.collapsed_collections.remove(&path) {
                    self.view.collapsed_collections.insert(path);
                }
                Task::none()
            }
            Message::ToggleEditMode => {
                let was_editing = matches!(self.view.edit_state, EditState::On { .. });
                self.view.edit_state = if was_editing {
                    EditState::Off
                } else {
                    EditState::On {
                        selection: HashSet::new(),
                    }
                };
                if was_editing {
                    self.persist_state();
                    if self.view.pending_rescan {
                        self.view.pending_rescan = false;
                        self.view.last_scan = Some(Instant::now());
                        return self.rescan_files();
                    }
                }
                Task::none()
            }
            Message::ToggleEditSelection(target) => {
                if let Some(selection) = edit_selection_mut(&mut self.view.edit_state)
                    && !selection.remove(&target)
                {
                    selection.insert(target);
                }
                Task::none()
            }
            Message::DeleteSelected => {
                let selection = match &self.view.edit_state {
                    EditState::On { selection } if !selection.is_empty() => selection.clone(),
                    _ => return Task::none(),
                };

                let mut remove_file_paths = Vec::new();
                let mut request_ids = Vec::new();

                for target in &selection {
                    match target {
                        EditTarget::Collection(path) => {
                            remove_file_paths.push(path.clone());
                        }
                        EditTarget::Request(id) => request_ids.push(id.clone()),
                    }
                }

                remove_file_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
                remove_file_paths.dedup();

                let remove_files_set: HashSet<PathBuf> =
                    remove_file_paths.iter().cloned().collect();

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
                    if let Some(file) = self.view.http_files.get_mut(&path) {
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
                        Err(err) => errors.push(format!(
                            "Failed to delete {}: {}",
                            path.display(),
                            err
                        )),
                    }
                    self.view.http_files.remove(path);
                }
                if !remove_file_paths.is_empty() {
                    self.view
                        .http_file_order
                        .retain(|path| !remove_files_set.contains(path));
                }

                if let EditState::On { selection } = &mut self.view.edit_state {
                    selection.clear();
                }
                if errors.is_empty() {
                    self.view.update_status_with_model("Deleted selection", &self.model);
                } else {
                    self.view
                        .update_status_with_model(&errors.join("; "), &self.model);
                }

                if let Some(RequestId::HttpFile { path, index }) = self.view.selection.clone()
                    && self
                        .view
                        .http_files
                        .get(&path)
                        .is_none_or(|file| index >= file.requests.len())
                {
                    self.view.selection = None;
                }

                Task::none()
            }
            Message::MoveCollectionUp(path) => {
                if let Some(pos) = self.view.http_file_order.iter().position(|p| p == &path)
                    && pos > 0
                {
                    self.view.http_file_order.swap(pos, pos - 1);
                }
                Task::none()
            }
            Message::MoveCollectionDown(path) => {
                if let Some(pos) = self.view.http_file_order.iter().position(|p| p == &path)
                    && pos + 1 < self.view.http_file_order.len()
                {
                    self.view.http_file_order.swap(pos, pos + 1);
                }
                Task::none()
            }
            Message::MoveRequestUp(id) => {
                let RequestId::HttpFile { path, index } = &id;
                if *index == 0 {
                    return Task::none();
                }
                let mut new_index = None;
                let mut status_error = None;
                if let Some(file) = self.view.http_files.get_mut(path)
                    && *index < file.requests.len()
                {
                    let updated_index = *index - 1;
                    file.requests.swap(*index, updated_index);
                    if let Err(err) = write_http_file(&file.path, &file.requests) {
                        file.requests.swap(*index, updated_index);
                        status_error = Some(format!(
                            "Failed to reorder {}: {}",
                            file.path.display(),
                            err
                        ));
                    } else {
                        new_index = Some(updated_index);
                    }
                }
                if let Some(updated_index) = new_index {
                    swap_request_indices_in_selection_http(
                        &mut self.view.selection,
                        path,
                        *index,
                        updated_index,
                    );
                    swap_request_indices_in_edit_selection_http(
                        &mut self.view.edit_state,
                        path,
                        *index,
                        updated_index,
                    );
                }
                if let Some(message) = status_error {
                    self.view.update_status_with_model(&message, &self.model);
                }
                Task::none()
            }
            Message::MoveRequestDown(id) => {
                let RequestId::HttpFile { path, index } = &id;
                let mut new_index = None;
                let mut status_error = None;
                if let Some(file) = self.view.http_files.get_mut(path)
                    && *index + 1 < file.requests.len()
                {
                    let updated_index = *index + 1;
                    file.requests.swap(*index, updated_index);
                    if let Err(err) = write_http_file(&file.path, &file.requests) {
                        file.requests.swap(*index, updated_index);
                        status_error = Some(format!(
                            "Failed to reorder {}: {}",
                            file.path.display(),
                            err
                        ));
                    } else {
                        new_index = Some(updated_index);
                    }
                }
                if let Some(updated_index) = new_index {
                    swap_request_indices_in_selection_http(
                        &mut self.view.selection,
                        path,
                        *index,
                        updated_index,
                    );
                    swap_request_indices_in_edit_selection_http(
                        &mut self.view.edit_state,
                        path,
                        *index,
                        updated_index,
                    );
                }
                if let Some(message) = status_error {
                    self.view.update_status_with_model(&message, &self.model);
                }
                Task::none()
            }
            Message::EnvironmentsLoaded(envs) => {
                self.view.set_environments(envs, &mut self.runtime.state);
                self.runtime.state.save();
                self.view.update_status_with_model("Ready", &self.model);
                Task::none()
            }
            Message::Select(id) => self.handle_selection(&id),
            Message::MethodSelected(method) => {
                self.apply_model_action(Action::MethodSelected(method), true, None)
            }
            Message::UrlChanged(url) => {
                self.apply_model_action(Action::UrlChanged(url), true, Some("Ready"))
            }
            Message::TitleChanged(title) => {
                self.apply_model_action(Action::TitleChanged(title), true, None)
            }
            Message::ModeChanged(mode) => {
                self.apply_model_action(Action::ModeChanged(mode), true, Some("Ready"))
            }
            Message::BodyEdited(action) => {
                self.apply_model_action(Action::BodyEdited(action), true, Some("Ready"))
            }
            Message::GraphqlQueryEdited(action) => {
                self.apply_model_action(Action::GraphqlQueryEdited(action), true, Some("Ready"))
            }
            Message::GraphqlVariablesEdited(action) => {
                self.apply_model_action(Action::GraphqlVariablesEdited(action), true, Some("Ready"))
            }
            Message::AuthChanged(new_auth) => {
                self.apply_model_action(Action::AuthChanged(new_auth), true, None)
            }
            Message::HeaderNameChanged(idx, value) => {
                let Some(index) = HeaderIndex::new(idx, self.model.header_rows.len()) else {
                    return Task::none();
                };
                self.apply_model_action(Action::HeaderNameChanged(index, value), true, Some("Ready"))
            }
            Message::HeaderValueChanged(idx, value) => {
                let Some(index) = HeaderIndex::new(idx, self.model.header_rows.len()) else {
                    return Task::none();
                };
                self.apply_model_action(Action::HeaderValueChanged(index, value), true, Some("Ready"))
            }
            Message::HeaderAdded => {
                self.apply_model_action(Action::HeaderAdded, true, Some("Ready"))
            }
            Message::HeaderRemoved(idx) => {
                let Some(index) = HeaderIndex::new(idx, self.model.header_rows.len()) else {
                    return Task::none();
                };
                self.apply_model_action(Action::HeaderRemoved(index), true, Some("Ready"))
            }
            Message::ResponseViewChanged(display) => {
                self.view.response_display = display;
                self.view.update_response_viewer();
                Task::none()
            }
            Message::ResponseTabChanged(tab) => {
                self.view.response_tab = tab;
                Task::none()
            }
            Message::ToggleShortcutsHelp => {
                self.view.show_shortcuts = !self.view.show_shortcuts;
                Task::none()
            }
            Message::CopyResponseBody => {
                let effect = Effect::CopyToClipboard(self.view.response_viewer.text());
                self.apply_model_action(Action::Emit(effect), false, None)
            }
            Message::CopyComplete => Task::none(),
            Message::AddRequest => self.handle_add_request(),
            Message::Send => self.handle_send(),
            Message::ResponseReady(result) => {
                match result {
                    Ok(resp) => {
                        self.view.update_status_with_model("Received response", &self.model);
                        self.view.last_response = Some(resp);
                    }
                    Err(err) => {
                        self.view.update_status_with_model("Request failed", &self.model);
                        self.view.last_response = Some(ResponsePreview::error(err));
                    }
                }
                self.view.update_response_viewer();
                Task::none()
            }
            Message::EnvironmentChanged(name) => {
                if let Some(index) = super::state::EnvironmentIndex::find(&name, &self.view.environments) {
                    self.view.active_environment = index;
                    self.runtime.state.active_environment = Some(name);
                    self.runtime.state.save();
                }
                self.view.update_status_with_model("Ready", &self.model);
                Task::none()
            }
            Message::Save => self.handle_save(),
            Message::Saved(result) => self.handle_saved(result),
            Message::SavePathChanged(path) => {
                self.apply_model_action(Action::SavePathChanged(path), true, None)
            }
            Message::Undo => self.handle_undo(),
            Message::Redo => self.handle_redo(),
        }
    }

    fn apply_model_action(
        &mut self,
        action: Action,
        record: bool,
        status_base: Option<&str>,
    ) -> Task<Message> {
        let focus = if record {
            Focus::from_selection(self.view.selection.as_ref())
        } else {
            Focus::None
        };
        let (model, effects) = reduce(std::mem::take(&mut self.model), action.clone());
        self.model = model;

        if record {
            self.history.record(action, focus, &self.model);
            let view = &self.view;
            self.history
                .trim_if_needed(|focus, model| apply_focus_with_view(view, focus, model));
        }

        if let Some(base) = status_base {
            self.view.update_status_with_model(base, &self.model);
        }

        self.run_effects(effects)
    }

    fn run_effects(&self, effects: Vec<Effect>) -> Task<Message> {
        if effects.is_empty() {
            return Task::none();
        }

        let tasks: Vec<Task<Message>> = effects
            .into_iter()
            .map(|effect| match effect {
                Effect::SendRequest { draft, env } => Task::perform(
                    send_request(self.runtime.client.clone(), draft, env),
                    Message::ResponseReady,
                ),
                Effect::PersistRequest {
                    root,
                    selection,
                    draft,
                    explicit_path,
                } => Task::perform(
                    async move {
                        persist_request(root, selection, draft, explicit_path)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::Saved,
                ),
                Effect::CopyToClipboard(text) => {
                    clipboard::write(text).map(|()| Message::CopyComplete)
                }
            })
            .collect();

        Task::batch(tasks)
    }

    fn persist_state(&mut self) {
        self.runtime.state.http_root = Some(self.view.http_root.clone());
        self.runtime.state.http_file_order = self.view.http_file_order.clone();
        self.runtime.state.save();
    }

    fn handle_selection(&mut self, id: &RequestId) -> Task<Message> {
        self.view.selection = Some(id.clone());
        if let Some(loaded) = self.view.resolve_request(id) {
            let task = self.apply_model_action(Action::LoadDraft(loaded), false, Some("Ready"));
            self.view.update_response_viewer();
            return task;
        }
        Task::none()
    }

    fn handle_add_request(&mut self) -> Task<Message> {
        if let Some(RequestId::HttpFile { path, .. }) = self.view.selection.clone() {
            let new_draft = RequestDraft {
                title: "New request".to_string(),
                ..Default::default()
            };
            if let Some(file) = self.view.http_files.get_mut(&path) {
                let persist_draft = new_draft.clone();
                file.requests.push(new_draft);
                let new_idx = file.requests.len() - 1;
                let new_id = RequestId::HttpFile {
                    path: path.clone(),
                    index: new_idx,
                };
                self.view.selection = Some(new_id.clone());
                if let Some(loaded) = self.view.resolve_request(&new_id) {
                    let task = self.apply_model_action(Action::LoadDraft(loaded), false, Some("Ready"));
                    self.view.update_response_viewer();
                    return task;
                }
                self.view.update_status_with_model("Saving new request...", &self.model);
                let effect = Effect::PersistRequest {
                    root: self.view.http_root.clone(),
                    selection: None,
                    draft: persist_draft,
                    explicit_path: Some(path),
                };
                return self.apply_model_action(Action::Emit(effect), false, None);
            }
            return Task::none();
        }

        self.view
            .update_status_with_model("Select a file to add a request", &self.model);
        Task::none()
    }

    fn handle_send(&mut self) -> Task<Message> {
        let env = self
            .view
            .environments
            .get(self.view.active_environment.get())
            .cloned();
        let (draft, extra_inputs) = self.prepare_send();
        self.view
            .update_status_with_draft("Sending...", &draft, &extra_inputs);
        let effect = Effect::SendRequest { draft, env };
        self.apply_model_action(Action::Emit(effect), false, None)
    }

    fn prepare_send(&self) -> (RequestDraft, Vec<String>) {
        let mut draft = self.model.draft.clone();
        let mut extra_inputs: Vec<String> = Vec::new();
        if self.model.mode == RequestMode::GraphQl {
            draft.method = Method::Post;
            let query = self.model.graphql_query.text();
            let variables = self.model.graphql_variables.text();
            extra_inputs.push(query.clone());
            extra_inputs.push(variables.clone());
            draft.body = build_graphql_body(&query, &variables);
            if !draft.headers.contains("Content-Type") {
                draft.headers.push_str("\nContent-Type: application/json");
            }
        }
        draft.headers = apply_auth_headers(&draft.headers, &self.model.auth);
        (draft, extra_inputs)
    }

    fn handle_save(&mut self) -> Task<Message> {
        let selection = self.view.selection.clone();
        let draft = self.model.draft.clone();
        let root = self.view.http_root.clone();
        let explicit_path = if let Some(RequestId::HttpFile { .. }) = selection {
            None
        } else {
            let path = self.model.save_path.trim();
            if path.is_empty() {
                self.view.update_status_with_model(
                    "Choose a path to save the request (Ctrl/Cmd+S)",
                    &self.model,
                );
                return Task::none();
            }
            Some(PathBuf::from(path))
        };
        self.view.update_status_with_model("Saving...", &self.model);
        let effect = Effect::PersistRequest {
            root,
            selection,
            draft,
            explicit_path,
        };
        self.apply_model_action(Action::Emit(effect), false, None)
    }

    fn handle_saved(&mut self, result: Result<(PathBuf, usize), String>) -> Task<Message> {
        match result {
            Ok((path, index)) => {
                let id = RequestId::HttpFile {
                    path: path.clone(),
                    index,
                };
                self.view.selection = Some(id);
                self.view
                    .update_status_with_model(&format!("Saved to {}", path.display()), &self.model);
                Task::batch([Task::none(), self.rescan_files()])
            }
            Err(err) => {
                self.view
                    .update_status_with_model(&format!("Save failed: {err}"), &self.model);
                Task::none()
            }
        }
    }

    fn handle_undo(&mut self) -> Task<Message> {
        let view = &self.view;
        if let Some(result) = self
            .history
            .undo(|focus, model| apply_focus_with_view(view, focus, model))
        {
            self.model = result.model;
            self.view.selection = result.focus.into_request();
            self.view.update_status_with_model("Ready", &self.model);
            self.view.update_response_viewer();
        }
        Task::none()
    }

    fn handle_redo(&mut self) -> Task<Message> {
        let view = &self.view;
        if let Some(result) = self
            .history
            .redo(|focus, model| apply_focus_with_view(view, focus, model))
        {
            self.model = result.model;
            self.view.selection = result.focus.into_request();
            self.view.update_status_with_model("Ready", &self.model);
            self.view.update_response_viewer();
        }
        Task::none()
    }
}

fn apply_focus_with_view(view: &ViewState, focus: &Focus, model: &mut AppModel) {
    let Some(id) = focus.request() else {
        return;
    };
    if let Some(loaded) = view.resolve_request(id) {
        model.load_draft(loaded);
    }
}
