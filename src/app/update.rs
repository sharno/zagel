use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use iced::widget::pane_grid;
use iced::{Task, clipboard};

use crate::model::{Method, RequestDraft, RequestId, ResponsePreview};
use crate::net::send_request;
use crate::parser::{persist_request, write_http_file};

use super::options::{RequestMode, apply_auth_headers, build_graphql_body};
use super::status::{status_with_missing, with_default_environment};
use super::{CollectionRef, EditState, EditTarget, HeaderRow, Message, Zagel};

const MIN_SPLIT_RATIO: f32 = 0.2;

fn clamp_ratio(ratio: f32) -> f32 {
    ratio.clamp(MIN_SPLIT_RATIO, 1.0 - MIN_SPLIT_RATIO)
}

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

const fn swap_collection_indices_in_selection(
    selection: &mut Option<RequestId>,
    a: usize,
    b: usize,
) {
    if let Some(RequestId::Collection { collection, .. }) = selection.as_mut() {
        if *collection == a {
            *collection = b;
        } else if *collection == b {
            *collection = a;
        }
    }
}

fn swap_collection_indices_in_edit_selection(edit_state: &mut EditState, a: usize, b: usize) {
    remap_edit_selection(edit_state, |item| match item {
        EditTarget::Collection(CollectionRef::CollectionIndex(idx)) => {
            if idx == a {
                EditTarget::Collection(CollectionRef::CollectionIndex(b))
            } else if idx == b {
                EditTarget::Collection(CollectionRef::CollectionIndex(a))
            } else {
                EditTarget::Collection(CollectionRef::CollectionIndex(idx))
            }
        }
        EditTarget::Request(RequestId::Collection { collection, index }) => {
            if collection == a {
                EditTarget::Request(RequestId::Collection { collection: b, index })
            } else if collection == b {
                EditTarget::Request(RequestId::Collection { collection: a, index })
            } else {
                EditTarget::Request(RequestId::Collection { collection, index })
            }
        }
        other => other,
    });
}

const fn swap_request_indices_in_selection_collection(
    selection: &mut Option<RequestId>,
    collection: usize,
    a: usize,
    b: usize,
) {
    if let Some(RequestId::Collection {
        collection: sel_collection,
        index,
    }) = selection.as_mut()
        && *sel_collection == collection
    {
        if *index == a {
            *index = b;
        } else if *index == b {
            *index = a;
        }
    }
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

fn swap_request_indices_in_edit_selection_collection(
    edit_state: &mut EditState,
    collection: usize,
    a: usize,
    b: usize,
) {
    remap_edit_selection(edit_state, |item| match item {
        EditTarget::Request(RequestId::Collection { collection: c, index }) => {
            if c == collection {
                if index == a {
                    EditTarget::Request(RequestId::Collection { collection: c, index: b })
                } else if index == b {
                    EditTarget::Request(RequestId::Collection { collection: c, index: a })
                } else {
                    EditTarget::Request(RequestId::Collection { collection: c, index })
                }
            } else {
                EditTarget::Request(RequestId::Collection { collection: c, index })
            }
        }
        other => other,
    });
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

#[allow(clippy::too_many_lines)]
impl Zagel {
    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.rescan_files(),
            Message::HttpFilesLoaded(files) => {
                self.http_files = files;
                self.http_file_order
                    .retain(|path| self.http_files.contains_key(path));
                let mut new_paths: Vec<PathBuf> = self
                    .http_files
                    .keys()
                    .filter(|path| !self.http_file_order.contains(path))
                    .cloned()
                    .collect();
                new_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
                self.http_file_order.extend(new_paths);
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
                self.edit_state = match self.edit_state {
                    EditState::Off => EditState::On {
                        selection: HashSet::new(),
                    },
                    EditState::On { .. } => EditState::Off,
                };
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
                let selection = match &self.edit_state {
                    EditState::On { selection } if !selection.is_empty() => selection.clone(),
                    _ => return Task::none(),
                };

                let mut remove_collection_indices = Vec::new();
                let mut remove_file_paths = Vec::new();
                let mut request_ids = Vec::new();

                for target in &selection {
                    match target {
                        EditTarget::Collection(CollectionRef::CollectionIndex(idx)) => {
                            remove_collection_indices.push(*idx);
                        }
                        EditTarget::Collection(CollectionRef::HttpFile(path)) => {
                            remove_file_paths.push(path.clone());
                        }
                        EditTarget::Request(id) => request_ids.push(id.clone()),
                    }
                }

                remove_collection_indices.sort_unstable();
                remove_collection_indices.dedup();
                remove_file_paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
                remove_file_paths.dedup();

                let remove_collections_set: HashSet<usize> =
                    remove_collection_indices.iter().copied().collect();
                let remove_files_set: HashSet<PathBuf> =
                    remove_file_paths.iter().cloned().collect();

                let mut collection_request_removals = std::collections::HashMap::new();
                let mut file_request_removals = std::collections::HashMap::new();

                for id in request_ids {
                    match id {
                        RequestId::Collection { collection, index } => {
                            if remove_collections_set.contains(&collection) {
                                continue;
                            }
                            collection_request_removals
                                .entry(collection)
                                .or_insert_with(Vec::new)
                                .push(index);
                        }
                        RequestId::HttpFile { path, index } => {
                            if remove_files_set.contains(&path) {
                                continue;
                            }
                            file_request_removals
                                .entry(path)
                                .or_insert_with(Vec::new)
                                .push(index);
                        }
                    }
                }

                let mut errors = Vec::new();

                for (collection, mut indices) in collection_request_removals {
                    if let Some(col) = self.collections.get_mut(collection) {
                        indices.sort_unstable();
                        indices.dedup();
                        for idx in indices.into_iter().rev() {
                            if idx < col.requests.len() {
                                col.requests.remove(idx);
                            }
                        }
                    }
                }

                for (path, mut indices) in file_request_removals {
                    if let Some(file) = self.http_files.get_mut(&path) {
                        indices.sort_unstable();
                        indices.dedup();
                        for idx in indices.into_iter().rev() {
                            if idx < file.requests.len() {
                                file.requests.remove(idx);
                            }
                        }
                        if let Err(err) = write_http_file(&file.path, &file.requests) {
                            errors.push(format!(
                                "Failed to update {}: {}",
                                file.path.display(),
                                err
                            ));
                        }
                    }
                }

                for idx in remove_collection_indices.into_iter().rev() {
                    if idx < self.collections.len() {
                        self.collections.remove(idx);
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
                    self.http_files.remove(path);
                }
                if !remove_file_paths.is_empty() {
                    self.http_file_order
                        .retain(|path| !remove_files_set.contains(path));
                }

                if let EditState::On { selection } = &mut self.edit_state {
                    selection.clear();
                }
                if errors.is_empty() {
                    self.update_status_with_missing("Deleted selection");
                } else {
                    self.update_status_with_missing(&errors.join("; "));
                }

                if let Some(selected) = self.selection.clone() {
                    let still_valid = match selected {
                        RequestId::Collection { collection, index } => self
                            .collections
                            .get(collection)
                            .is_some_and(|col| index < col.requests.len()),
                        RequestId::HttpFile { path, index } => self
                            .http_files
                            .get(&path)
                            .is_some_and(|file| index < file.requests.len()),
                    };
                    if !still_valid {
                        self.selection = None;
                    }
                }

                Task::none()
            }
            Message::MoveCollectionUp(collection_ref) => {
                match collection_ref {
                    CollectionRef::CollectionIndex(idx) => {
                        if idx > 0 && idx < self.collections.len() {
                            self.collections.swap(idx, idx - 1);
                            swap_collection_indices_in_selection(&mut self.selection, idx, idx - 1);
                            swap_collection_indices_in_edit_selection(
                                &mut self.edit_state,
                                idx,
                                idx - 1,
                            );
                        }
                    }
                    CollectionRef::HttpFile(path) => {
                        if let Some(pos) =
                            self.http_file_order.iter().position(|p| p == &path)
                            && pos > 0
                        {
                            self.http_file_order.swap(pos, pos - 1);
                        }
                    }
                }
                Task::none()
            }
            Message::MoveCollectionDown(collection_ref) => {
                match collection_ref {
                    CollectionRef::CollectionIndex(idx) => {
                        if idx + 1 < self.collections.len() {
                            self.collections.swap(idx, idx + 1);
                            swap_collection_indices_in_selection(&mut self.selection, idx, idx + 1);
                            swap_collection_indices_in_edit_selection(
                                &mut self.edit_state,
                                idx,
                                idx + 1,
                            );
                        }
                    }
                    CollectionRef::HttpFile(path) => {
                        if let Some(pos) =
                            self.http_file_order.iter().position(|p| p == &path)
                            && pos + 1 < self.http_file_order.len()
                        {
                            self.http_file_order.swap(pos, pos + 1);
                        }
                    }
                }
                Task::none()
            }
            Message::MoveRequestUp(id) => {
                match &id {
                    RequestId::Collection { collection, index } => {
                        if *index == 0 {
                            return Task::none();
                        }
                        if let Some(col) = self.collections.get_mut(*collection)
                            && *index < col.requests.len()
                        {
                            let new_index = *index - 1;
                            col.requests.swap(*index, new_index);
                            swap_request_indices_in_selection_collection(
                                &mut self.selection,
                                *collection,
                                *index,
                                new_index,
                            );
                            swap_request_indices_in_edit_selection_collection(
                                &mut self.edit_state,
                                *collection,
                                *index,
                                new_index,
                            );
                        }
                    }
                    RequestId::HttpFile { path, index } => {
                        if *index == 0 {
                            return Task::none();
                        }
                        let mut new_index = None;
                        let mut status_error = None;
                        if let Some(file) = self.http_files.get_mut(path)
                            && *index < file.requests.len()
                        {
                            let updated_index = *index - 1;
                            file.requests.swap(*index, updated_index);
                            if let Err(err) = write_http_file(&file.path, &file.requests) {
                                status_error = Some(format!(
                                    "Failed to reorder {}: {}",
                                    file.path.display(),
                                    err
                                ));
                            }
                            new_index = Some(updated_index);
                        }
                        if let Some(updated_index) = new_index {
                            swap_request_indices_in_selection_http(
                                &mut self.selection,
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
                    }
                }
                Task::none()
            }
            Message::MoveRequestDown(id) => {
                match &id {
                    RequestId::Collection { collection, index } => {
                        if let Some(col) = self.collections.get_mut(*collection)
                            && *index + 1 < col.requests.len()
                        {
                            let new_index = *index + 1;
                            col.requests.swap(*index, new_index);
                            swap_request_indices_in_selection_collection(
                                &mut self.selection,
                                *collection,
                                *index,
                                new_index,
                            );
                            swap_request_indices_in_edit_selection_collection(
                                &mut self.edit_state,
                                *collection,
                                *index,
                                new_index,
                            );
                        }
                    }
                    RequestId::HttpFile { path, index } => {
                        let mut new_index = None;
                        let mut status_error = None;
                        if let Some(file) = self.http_files.get_mut(path)
                            && *index + 1 < file.requests.len()
                        {
                            let updated_index = *index + 1;
                            file.requests.swap(*index, updated_index);
                            if let Err(err) = write_http_file(&file.path, &file.requests) {
                                status_error = Some(format!(
                                    "Failed to reorder {}: {}",
                                    file.path.display(),
                                    err
                                ));
                            }
                            new_index = Some(updated_index);
                        }
                        if let Some(updated_index) = new_index {
                            swap_request_indices_in_selection_http(
                                &mut self.selection,
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
                    }
                }
                Task::none()
            }
            Message::EnvironmentsLoaded(envs) => {
                self.environments = with_default_environment(envs);
                self.apply_saved_environment();
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
            Message::CopyResponseBody => {
                clipboard::write(self.response_viewer.text()).map(|()| Message::CopyComplete)
            }
            Message::CopyComplete => Task::none(),
            Message::AddRequest => {
                let new_draft = RequestDraft {
                    title: "New request".to_string(),
                    ..Default::default()
                };
                if let Some(RequestId::Collection { collection, .. }) = self.selection {
                    if let Some(col) = self.collections.get_mut(collection) {
                        col.requests.push(new_draft);
                        let new_idx = col.requests.len() - 1;
                        let new_id = RequestId::Collection {
                            collection,
                            index: new_idx,
                        };
                        self.apply_selection(&new_id);
                        return Task::none();
                    }
                } else if let Some(RequestId::HttpFile { path, .. }) = self.selection.clone()
                    && let Some(file) = self.http_files.get_mut(&path)
                {
                    file.requests.push(new_draft.clone());
                    let new_idx = file.requests.len() - 1;
                    let new_id = RequestId::HttpFile {
                        path: path.clone(),
                        index: new_idx,
                    };
                    self.selection = Some(new_id);
                    self.update_status_with_missing("Saving new request...");
                    let http_root = self.http_root.clone();
                    return Task::perform(
                        async move {
                            persist_request(http_root, None, new_draft, Some(path))
                                .await
                                .map_err(|e| e.to_string())
                        },
                        Message::Saved,
                    );
                }
                self.update_status_with_missing("Select a collection to add a request");
                Task::none()
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
                let extra_refs: Vec<&str> = extra_inputs
                    .iter()
                    .map(std::string::String::as_str)
                    .collect();
                self.status_line =
                    status_with_missing("Sending...", &draft, env.as_ref(), &extra_refs);
                Task::perform(
                    send_request(self.client.clone(), draft, env),
                    Message::ResponseReady,
                )
            }
            Message::ResponseReady(result) => {
                match result {
                    Ok(resp) => {
                        self.update_status_with_missing("Received response");
                        self.last_response = Some(resp);
                    }
                    Err(err) => {
                        self.update_status_with_missing("Request failed");
                        self.last_response = Some(ResponsePreview::error(err));
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
                let selection = self.selection.clone();
                let draft = self.draft.clone();
                let root = self.http_root.clone();
                let explicit_path = if let Some(RequestId::HttpFile { .. }) = selection {
                    None
                } else {
                    let path = self.save_path.trim();
                    if path.is_empty() {
                        self.update_status_with_missing(
                            "Choose a path to save the request (Ctrl/Cmd+S)",
                        );
                        return Task::none();
                    }
                    Some(PathBuf::from(path))
                };
                self.update_status_with_missing("Saving...");
                Task::perform(
                    async move {
                        persist_request(root, selection, draft, explicit_path)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::Saved,
                )
            }
            Message::Saved(result) => match result {
                Ok((path, index)) => {
                    let id = RequestId::HttpFile {
                        path: path.clone(),
                        index,
                    };
                    self.selection = Some(id);
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
        }
    }
}
