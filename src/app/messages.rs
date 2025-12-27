use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::pane_grid;
use iced::widget::text_editor;

use crate::model::{Environment, HttpFile, Method, RequestId, ResponsePreview};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CollectionRef {
    CollectionIndex(usize),
    HttpFile(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditTarget {
    Collection(CollectionRef),
    Request(RequestId),
}

#[derive(Debug, Clone)]
pub enum Message {
    HttpFilesLoaded(HashMap<PathBuf, HttpFile>),
    EnvironmentsLoaded(Vec<Environment>),
    Tick,
    Select(RequestId),
    MethodSelected(Method),
    UrlChanged(String),
    TitleChanged(String),
    BodyEdited(text_editor::Action),
    Send,
    ResponseReady(Result<ResponsePreview, String>),
    EnvironmentChanged(String),
    Save,
    Saved(Result<(PathBuf, usize), String>),
    SavePathChanged(String),
    ModeChanged(crate::app::options::RequestMode),
    GraphqlQueryEdited(text_editor::Action),
    GraphqlVariablesEdited(text_editor::Action),
    AuthChanged(crate::app::options::AuthState),
    HeaderNameChanged(usize, String),
    HeaderValueChanged(usize, String),
    HeaderAdded,
    HeaderRemoved(usize),
    ResponseViewChanged(crate::app::view::ResponseDisplay),
    ResponseTabChanged(crate::app::view::ResponseTab),
    CopyResponseBody,
    CopyComplete,
    PaneResized(pane_grid::ResizeEvent),
    WorkspacePaneResized(pane_grid::ResizeEvent),
    BuilderPaneResized(pane_grid::ResizeEvent),
    ToggleCollection(String),
    ToggleEditMode,
    ToggleEditSelection(EditTarget),
    DeleteSelected,
    MoveCollectionUp(CollectionRef),
    MoveCollectionDown(CollectionRef),
    MoveRequestUp(RequestId),
    MoveRequestDown(RequestId),
    AddRequest,
    ToggleShortcutsHelp,
}
