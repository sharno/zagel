use std::path::PathBuf;

use iced::widget::pane_grid;
use iced::{Subscription, Task, Theme, application};
use reqwest::Client;

use crate::parser::{scan_env_files, scan_http_files};
use crate::state::AppState;

use super::history::History;
use super::state::{AppModel, Runtime, ViewState};
use super::{Message, hotkeys, view, watcher};

const FILE_SCAN_MAX_DEPTH: usize = 6;

pub struct Zagel {
    pub(super) model: AppModel,
    pub(super) view: ViewState,
    pub(super) runtime: Runtime,
    pub(super) history: History,
}

impl Zagel {
    pub(super) fn init() -> (Self, Task<Message>) {
        let mut state = AppState::load();
        let http_root = state
            .http_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let (mut panes, sidebar) = pane_grid::State::new(super::view::PaneContent::Sidebar);
        let split = panes.split(
            pane_grid::Axis::Vertical,
            sidebar,
            super::view::PaneContent::Workspace,
        );
        if let Some((_, split)) = split {
            panes.resize(split, 0.26);
        }

        let (mut workspace_panes, builder) =
            pane_grid::State::new(super::view::WorkspacePane::Builder);
        if let Some((_, split)) = workspace_panes.split(
            pane_grid::Axis::Horizontal,
            builder,
            super::view::WorkspacePane::Response,
        ) {
            workspace_panes.resize(split, 0.62);
        }

        let (mut builder_panes, form) = pane_grid::State::new(super::view::BuilderPane::Form);
        if let Some((_, split)) = builder_panes.split(
            pane_grid::Axis::Vertical,
            form,
            super::view::BuilderPane::Body,
        ) {
            builder_panes.resize(split, 0.45);
        }

        let model = AppModel::default();
        let http_file_order = state.http_file_order.clone();
        let mut view_state = ViewState::new(
            http_root.clone(),
            panes,
            workspace_panes,
            builder_panes,
            http_file_order,
        );
        view_state.update_status_with_model("Ready", &model);

        state.http_root = Some(http_root);
        state.save();

        let runtime = Runtime {
            client: Client::new(),
            state,
        };
        let history = History::new(model.clone());

        let app = Self {
            model,
            view: view_state,
            runtime,
            history,
        };

        let task = app.rescan_files();
        (app, task)
    }

    pub(super) fn subscription(state: &Self) -> Subscription<Message> {
        Subscription::batch([
            hotkeys::subscription(),
            watcher::subscription(state.view.http_root.clone()),
        ])
    }

    pub(super) const fn theme(state: &Self) -> Theme {
        state.runtime.state.theme.iced_theme()
    }

    pub(super) fn rescan_files(&self) -> Task<Message> {
        Task::batch([
            Task::perform(
                scan_http_files(self.view.http_root.clone(), FILE_SCAN_MAX_DEPTH),
                Message::HttpFilesLoaded,
            ),
            Task::perform(
                scan_env_files(self.view.http_root.clone(), FILE_SCAN_MAX_DEPTH),
                Message::EnvironmentsLoaded,
            ),
        ])
    }
}

pub fn run() -> iced::Result {
    application(Zagel::init, Zagel::update, view::view)
        .title("Zagel  REST workbench")
        .subscription(Zagel::subscription)
        .theme(Zagel::theme)
        .run()
}
