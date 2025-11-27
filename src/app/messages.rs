use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::text_editor;

use crate::model::{Environment, HttpFile, Method, RequestId, ResponsePreview};

#[derive(Debug, Clone)]
pub enum Message {
    HttpFilesLoaded(HashMap<PathBuf, HttpFile>),
    EnvironmentsLoaded(Vec<Environment>),
    Tick,
    Select(RequestId),
    MethodSelected(Method),
    UrlChanged(String),
    TitleChanged(String),
    HeadersEdited(text_editor::Action),
    BodyEdited(text_editor::Action),
    AddUnsavedTab,
    Send,
    ResponseReady(Result<ResponsePreview, String>),
    EnvironmentChanged(String),
    Save,
    Saved(Result<(PathBuf, usize), String>),
    SavePathChanged(String),
}
