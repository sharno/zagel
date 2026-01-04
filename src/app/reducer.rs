use std::path::PathBuf;

use iced::widget::text_editor;

use crate::model::{Environment, Method, RequestDraft, RequestId};

use super::options::{AuthState, RequestMode};
use super::state::{AppModel, HeaderRow, LoadedDraft};

#[derive(Debug, Clone, Copy)]
pub struct HeaderIndex(usize);

impl HeaderIndex {
    pub const fn new(index: usize, row_count: usize) -> Option<Self> {
        if index < row_count {
            Some(Self(index))
        } else {
            None
        }
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    LoadDraft(LoadedDraft),
    MethodSelected(Method),
    UrlChanged(String),
    TitleChanged(String),
    ModeChanged(RequestMode),
    BodyEdited(text_editor::Action),
    GraphqlQueryEdited(text_editor::Action),
    GraphqlVariablesEdited(text_editor::Action),
    AuthChanged(AuthState),
    HeaderNameChanged(HeaderIndex, String),
    HeaderValueChanged(HeaderIndex, String),
    HeaderAdded,
    HeaderRemoved(HeaderIndex),
    SavePathChanged(String),
    Emit(Effect),
}

#[derive(Debug, Clone)]
pub enum Effect {
    RescanFiles,
    SendRequest {
        draft: RequestDraft,
        env: Option<Environment>,
    },
    PersistRequest {
        root: PathBuf,
        selection: Option<RequestId>,
        draft: RequestDraft,
        explicit_path: Option<PathBuf>,
    },
    CopyToClipboard(String),
}

pub fn reduce(mut model: AppModel, action: Action) -> (AppModel, Vec<Effect>) {
    match action {
        Action::LoadDraft(loaded) => {
            model.load_draft(loaded);
            (model, Vec::new())
        }
        Action::MethodSelected(method) => {
            model.draft.method = method;
            (model, Vec::new())
        }
        Action::UrlChanged(url) => {
            model.draft.url = url;
            (model, Vec::new())
        }
        Action::TitleChanged(title) => {
            model.draft.title = title;
            (model, Vec::new())
        }
        Action::ModeChanged(mode) => {
            model.mode = mode;
            (model, Vec::new())
        }
        Action::BodyEdited(action) => {
            model.body_editor.perform(action);
            model.draft.body = model.body_editor.text();
            (model, Vec::new())
        }
        Action::GraphqlQueryEdited(action) => {
            model.graphql_query.perform(action);
            (model, Vec::new())
        }
        Action::GraphqlVariablesEdited(action) => {
            model.graphql_variables.perform(action);
            (model, Vec::new())
        }
        Action::AuthChanged(auth) => {
            model.auth = auth;
            (model, Vec::new())
        }
        Action::HeaderNameChanged(idx, value) => {
            if let Some(row) = model.header_rows.get_mut(idx.get()) {
                row.name = value;
                model.rebuild_headers_from_rows();
            }
            (model, Vec::new())
        }
        Action::HeaderValueChanged(idx, value) => {
            if let Some(row) = model.header_rows.get_mut(idx.get()) {
                row.value = value;
                model.rebuild_headers_from_rows();
            }
            (model, Vec::new())
        }
        Action::HeaderAdded => {
            model.header_rows.push(HeaderRow {
                name: String::new(),
                value: String::new(),
            });
            model.rebuild_headers_from_rows();
            (model, Vec::new())
        }
        Action::HeaderRemoved(idx) => {
            if idx.get() < model.header_rows.len() {
                model.header_rows.remove(idx.get());
                model.rebuild_headers_from_rows();
            }
            (model, Vec::new())
        }
        Action::SavePathChanged(path) => {
            model.save_path = path;
            (model, Vec::new())
        }
        Action::Emit(effect) => (model, vec![effect]),
    }
}
