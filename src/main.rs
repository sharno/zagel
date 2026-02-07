#![allow(clippy::multiple_crate_versions)]

mod app;
mod model;
mod net;
mod parser;
mod pathing;
mod state;
mod theme;

fn main() -> iced::Result {
    app::run()
}
