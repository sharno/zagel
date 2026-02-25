#![allow(clippy::multiple_crate_versions)]

mod app;
mod cli;
mod icons;
mod launch;
mod model;
mod net;
mod parser;
mod pathing;
mod state;
mod theme;

fn main() -> iced::Result {
    let launch = match cli::parse_env() {
        Ok(launch) => launch,
        Err(cli::CliError::HelpRequested) => {
            println!("{}", cli::usage());
            return Ok(());
        }
        Err(err) => {
            eprintln!("{err}");
            eprintln!("{}", cli::usage());
            std::process::exit(2);
        }
    };

    if let Some(path) = launch.state_file.clone()
        && let Err(_existing) = state::set_state_file_override(path)
    {
        eprintln!("state file override was already configured");
        std::process::exit(2);
    }

    app::run(launch)
}
