use std::path::PathBuf;

use iced::widget::pane_grid;
use iced::{Task, clipboard};

use crate::model::{Method, RequestDraft, RequestId, ResponsePreview};
use crate::net::send_request;
use crate::parser::persist_request;

use super::options::{RequestMode, apply_auth_headers, build_graphql_body};
use super::status::{status_with_missing, with_default_environment};
use super::{HeaderRow, Message, Zagel};

const MIN_SPLIT_RATIO: f32 = 0.2;

fn clamp_ratio(ratio: f32) -> f32 {
    ratio.clamp(MIN_SPLIT_RATIO, 1.0 - MIN_SPLIT_RATIO)
}

#[allow(clippy::too_many_lines)]
impl Zagel {
    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.rescan_files(),
            Message::HttpFilesLoaded(files) => {
                self.http_files = files;
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
                self.save_undo_state();
                self.draft.method = method;
                Task::none()
            }
            Message::UrlChanged(url) => {
                self.save_undo_state();
                self.draft.url = url;
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::TitleChanged(title) => {
                self.save_undo_state();
                self.draft.title = title;
                Task::none()
            }
            Message::ModeChanged(mode) => {
                self.save_undo_state();
                self.mode = mode;
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::BodyEdited(action) => {
                self.save_undo_state();
                self.body_editor.perform(action);
                self.draft.body = self.body_editor.text();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::GraphqlQueryEdited(action) => {
                self.save_undo_state();
                self.graphql_query.perform(action);
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::GraphqlVariablesEdited(action) => {
                self.save_undo_state();
                self.graphql_variables.perform(action);
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::AuthChanged(new_auth) => {
                self.save_undo_state();
                self.auth = new_auth;
                Task::none()
            }
            Message::HeaderNameChanged(idx, value) => {
                self.save_undo_state();
                if let Some(row) = self.header_rows.get_mut(idx) {
                    row.name = value;
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderValueChanged(idx, value) => {
                self.save_undo_state();
                if let Some(row) = self.header_rows.get_mut(idx) {
                    row.value = value;
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderAdded => {
                self.save_undo_state();
                self.header_rows.push(HeaderRow {
                    name: String::new(),
                    value: String::new(),
                });
                self.rebuild_headers_from_rows();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderRemoved(idx) => {
                self.save_undo_state();
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
            Message::Undo => {
                if let Some(state) = self.undo_stack.pop() {
                    let current_state = super::lifecycle::UndoableState {
                        draft: self.draft.clone(),
                        body_text: self.body_editor.text(),
                        header_rows: self.header_rows.clone(),
                    };
                    self.redo_stack.push(current_state);

                    self.restore_state(&state);
                    self.update_status_with_missing("Undo");
                } else {
                    self.update_status_with_missing("Nothing to undo");
                }
                Task::none()
            }
            Message::Redo => {
                if let Some(state) = self.redo_stack.pop() {
                    let current_state = super::lifecycle::UndoableState {
                        draft: self.draft.clone(),
                        body_text: self.body_editor.text(),
                        header_rows: self.header_rows.clone(),
                    };
                    self.undo_stack.push(current_state);

                    self.restore_state(&state);
                    self.update_status_with_missing("Redo");
                } else {
                    self.update_status_with_missing("Nothing to redo");
                }
                Task::none()
            }
        }
    }
}
