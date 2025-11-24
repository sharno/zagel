#![allow(clippy::multiple_crate_versions)]

mod app;
mod model;
mod net;
mod parser;
mod state;

fn main() -> iced::Result {
    app::run()
}
