use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::pane_grid;
use iced::widget::text_editor;

use crate::model::{Environment, HttpFile, Method, RequestId};
use crate::pathing::{GlobalEnvRoot, ProjectRoot};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditTarget {
    Collection(PathBuf),
    Request(RequestId),
}

#[derive(Debug, Clone)]
pub enum Message {
    HttpFilesLoaded(HashMap<PathBuf, HttpFile>),
    EnvironmentsLoaded(Vec<Environment>),
    FilesChanged,
    WatcherUnavailable(String),
    Select(RequestId),
    MethodSelected(Method),
    UrlChanged(String),
    TitleChanged(String),
    BodyEdited(text_editor::Action),
    Send,
    ResponseReady(Result<crate::net::SendOutcome, String>),
    EnvironmentChanged(String),
    Save,
    Saved(Result<(PathBuf, usize), String>),
    SavePathChanged(String),
    ProjectPathInputChanged(String),
    AddProject,
    RemoveProject(ProjectRoot),
    GlobalEnvPathInputChanged(String),
    AddGlobalEnvRoot,
    RemoveGlobalEnvRoot(GlobalEnvRoot),
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
    CopyResponseRaw,
    CopyResponsePretty,
    CopyComplete,
    PaneResized(pane_grid::ResizeEvent),
    WorkspacePaneResized(pane_grid::ResizeEvent),
    BuilderPaneResized(pane_grid::ResizeEvent),
    ToggleCollection(String),
    ToggleEditMode,
    ToggleEditSelection(EditTarget),
    DeleteSelected,
    MoveCollectionUp(PathBuf),
    MoveCollectionDown(PathBuf),
    MoveRequestUp(RequestId),
    MoveRequestDown(RequestId),
    AddRequest,
    ToggleShortcutsHelp,
    AutomationStart,
    AutomationPoll,
    AutomationWindowResolved(Option<iced::window::Id>),
    AutomationScreenshotCaptured(iced::window::Screenshot),
}
