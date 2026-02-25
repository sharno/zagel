use std::ffi::OsString;
use std::fmt::{Display, Formatter};
use std::io;
use std::path::PathBuf;

use crate::launch::{AutomationOptions, LaunchOptions};

const DEFAULT_SCREENSHOT_DIR: &str = "artifacts/ui";

#[derive(Debug)]
pub enum CliError {
    HelpRequested,
    MissingValue(&'static str),
    UnknownFlag(String),
    NonUtf8Flag,
    CurrentDirectory(io::Error),
    MissingAutomationScenario,
}

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HelpRequested => f.write_str("help requested"),
            Self::MissingValue(flag) => write!(f, "missing value for {flag}"),
            Self::UnknownFlag(flag) => write!(f, "unknown flag: {flag}"),
            Self::NonUtf8Flag => f.write_str("encountered a non-UTF8 argument"),
            Self::CurrentDirectory(err) => {
                write!(f, "failed to read current directory: {err}")
            }
            Self::MissingAutomationScenario => {
                f.write_str("automation flags were provided without --automation <scenario.toml>")
            }
        }
    }
}

impl std::error::Error for CliError {}

pub const fn usage() -> &'static str {
    "Usage: zagel [OPTIONS]\n\n\
Options:\n\
  --state-file <path>          Override persisted state path\n\
  --project-root <path>        Add project root override (repeatable)\n\
  --global-env-root <path>     Add global env root override (repeatable)\n\
  --automation <path>          Run automation scenario from TOML file\n\
  --screenshot-dir <path>      Output directory for automation screenshots\n\
  --automation-state-out <path> Write full automation state snapshot (JSON)\n\
  --exit-when-done             Exit app when automation scenario completes\n\
  -h, --help                   Show this help\n"
}

pub fn parse_env() -> Result<LaunchOptions, CliError> {
    parse_args(std::env::args_os().skip(1))
}

fn resolve_path(raw: OsString) -> Result<PathBuf, CliError> {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(CliError::CurrentDirectory)
    }
}

fn next_path(
    iter: &mut impl Iterator<Item = OsString>,
    flag: &'static str,
) -> Result<PathBuf, CliError> {
    let raw = iter.next().ok_or(CliError::MissingValue(flag))?;
    if raw.to_str().is_some_and(|value| value.starts_with('-')) {
        return Err(CliError::MissingValue(flag));
    }
    resolve_path(raw)
}

pub fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<LaunchOptions, CliError> {
    let mut options = LaunchOptions::default();
    let mut automation_scenario: Option<PathBuf> = None;
    let mut screenshot_dir: Option<PathBuf> = None;
    let mut state_output_path: Option<PathBuf> = None;
    let mut exit_when_done = false;

    let mut iter = args.into_iter();
    while let Some(raw_flag) = iter.next() {
        let Some(flag) = raw_flag.to_str() else {
            return Err(CliError::NonUtf8Flag);
        };

        match flag {
            "-h" | "--help" => return Err(CliError::HelpRequested),
            "--state-file" => {
                options.state_file = Some(next_path(&mut iter, "--state-file")?);
            }
            "--project-root" => {
                options
                    .project_roots
                    .push(next_path(&mut iter, "--project-root")?);
            }
            "--global-env-root" => {
                options
                    .global_env_roots
                    .push(next_path(&mut iter, "--global-env-root")?);
            }
            "--automation" => {
                automation_scenario = Some(next_path(&mut iter, "--automation")?);
            }
            "--screenshot-dir" => {
                screenshot_dir = Some(next_path(&mut iter, "--screenshot-dir")?);
            }
            "--automation-state-out" => {
                state_output_path = Some(next_path(&mut iter, "--automation-state-out")?);
            }
            "--exit-when-done" => {
                exit_when_done = true;
            }
            _ => {
                return Err(CliError::UnknownFlag(flag.to_string()));
            }
        }
    }

    if automation_scenario.is_some()
        || screenshot_dir.is_some()
        || state_output_path.is_some()
        || exit_when_done
    {
        let scenario_path = automation_scenario.ok_or(CliError::MissingAutomationScenario)?;
        let screenshot_dir = match screenshot_dir {
            Some(dir) => dir,
            None => resolve_path(OsString::from(DEFAULT_SCREENSHOT_DIR))?,
        };
        options.automation = Some(AutomationOptions {
            scenario_path,
            screenshot_dir,
            state_output_path,
            exit_when_done,
        });
    }

    Ok(options)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{CliError, parse_args};

    #[test]
    fn parses_automation_state_output_flag() {
        let args = vec![
            OsString::from("--automation"),
            OsString::from("./tests/ui/scenarios/smoke.toml"),
            OsString::from("--automation-state-out"),
            OsString::from("./artifacts/ui/state.json"),
            OsString::from("--exit-when-done"),
        ];

        let parsed = parse_args(args).expect("parse args");
        let automation = parsed
            .automation
            .expect("automation options should be present");

        assert_eq!(
            automation
                .state_output_path
                .expect("state output path should be parsed")
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("state.json")
        );
        assert!(automation.exit_when_done);
    }

    #[test]
    fn automation_related_flags_require_automation_scenario() {
        let args = vec![
            OsString::from("--automation-state-out"),
            OsString::from("out.json"),
        ];

        let err = parse_args(args).expect_err("missing automation scenario should fail");
        assert!(matches!(err, CliError::MissingAutomationScenario));
    }

    #[test]
    fn missing_value_is_reported_when_next_token_is_a_flag() {
        let args = vec![
            OsString::from("--state-file"),
            OsString::from("--project-root"),
            OsString::from("./workspace"),
        ];

        let err = parse_args(args).expect_err("missing value should fail");
        assert!(matches!(err, CliError::MissingValue("--state-file")));
    }
}
