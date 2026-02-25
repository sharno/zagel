use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Deserialize;
use tempfile::{tempdir, TempDir};

#[derive(Debug, Deserialize)]
struct SnapshotProgress {
    current_step: usize,
    total_steps: usize,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct SelectedRequest {
    path: String,
    index: usize,
}

#[derive(Debug, Deserialize)]
struct StateSnapshot {
    scenario_name: String,
    status_line: String,
    outcome: String,
    progress: SnapshotProgress,
    selected_request: Option<SelectedRequest>,
}

struct ScenarioRunPaths {
    _temp: Option<TempDir>,
    state_file: PathBuf,
    artifacts: PathBuf,
    state_json: PathBuf,
}

fn e2e_enabled() -> bool {
    std::env::var("ZAGEL_E2E")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn binary_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_zagel") {
        return Some(PathBuf::from(path));
    }

    let mut fallback = manifest_path();
    fallback.push("target");
    fallback.push("debug");
    fallback.push(if cfg!(windows) { "zagel.exe" } else { "zagel" });
    fallback.exists().then_some(fallback)
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn require_file(path: &Path) {
    assert!(path.exists(), "expected file to exist: {}", path.display());
}

fn prepare_run_paths(test_name: &str) -> ScenarioRunPaths {
    if let Some(base) = std::env::var_os("ZAGEL_E2E_ARTIFACTS_DIR") {
        let base = PathBuf::from(base).join(test_name);
        if base.exists() {
            fs::remove_dir_all(&base).expect("remove previous test artifact directory");
        }
        fs::create_dir_all(&base).expect("create test artifact directory");

        let artifacts = base.join("artifacts");
        fs::create_dir_all(&artifacts).expect("create screenshots artifact directory");

        let state_json = artifacts.join("state.json");
        let state_file = base.join("state.toml");
        return ScenarioRunPaths {
            _temp: None,
            state_file,
            artifacts,
            state_json,
        };
    }

    let temp = tempdir().expect("create temp dir for e2e outputs");
    let state_file = temp.path().join("state.toml");
    let artifacts = temp.path().join("artifacts");
    let state_json = artifacts.join("state.json");

    ScenarioRunPaths {
        _temp: Some(temp),
        state_file,
        artifacts,
        state_json,
    }
}

fn run_scenario(
    binary: &Path,
    root: &Path,
    fixture_workspace: &Path,
    scenario: &Path,
    paths: &ScenarioRunPaths,
) -> Output {
    Command::new(binary)
        .current_dir(root)
        .arg("--state-file")
        .arg(&paths.state_file)
        .arg("--project-root")
        .arg(fixture_workspace)
        .arg("--automation")
        .arg(scenario)
        .arg("--screenshot-dir")
        .arg(&paths.artifacts)
        .arg("--automation-state-out")
        .arg(&paths.state_json)
        .arg("--exit-when-done")
        .output()
        .expect("run automation scenario")
}

fn read_snapshot(path: &Path) -> StateSnapshot {
    serde_json::from_str(&fs::read_to_string(path).expect("read automation state snapshot"))
        .expect("parse automation state snapshot json")
}

#[test]
fn automation_navigation_scenario_emits_screenshots_and_state() {
    if !e2e_enabled() {
        eprintln!("skipping UI e2e test (set ZAGEL_E2E=1 to enable)");
        return;
    }

    let Some(binary) = binary_path() else {
        eprintln!("skipping UI e2e test (zagel binary not available in test env)");
        return;
    };

    let root = manifest_path();
    let fixture_workspace = root.join("tests/ui/fixtures/workspace");
    let scenario = root.join("tests/ui/scenarios/ui_navigation.toml");
    let paths = prepare_run_paths("automation_navigation_scenario");

    let output = run_scenario(&binary, &root, &fixture_workspace, &scenario, &paths);

    assert!(
        output.status.success(),
        "automation run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    require_file(&paths.artifacts.join("02-selected.png"));
    require_file(&paths.artifacts.join("04-ready.png"));
    require_file(&paths.state_json);

    let snapshot = read_snapshot(&paths.state_json);

    assert_eq!(snapshot.outcome, "completed");
    assert_eq!(snapshot.scenario_name, "ui-navigation");
    assert!(snapshot.status_line.contains("completed"));
    assert_eq!(
        snapshot.progress.current_step,
        snapshot.progress.total_steps
    );
    assert!(snapshot.progress.done);

    let selected = snapshot
        .selected_request
        .expect("snapshot should include selected request");
    assert!(selected.path.contains("sample.http"));
    assert_eq!(selected.index, 0);
}

#[test]
fn snapshot_only_scenario_emits_final_state_without_screenshots() {
    if !e2e_enabled() {
        eprintln!("skipping UI e2e test (set ZAGEL_E2E=1 to enable)");
        return;
    }

    let Some(binary) = binary_path() else {
        eprintln!("skipping UI e2e test (zagel binary not available in test env)");
        return;
    };

    let root = manifest_path();
    let fixture_workspace = root.join("tests/ui/fixtures/workspace");
    let scenario = root.join("tests/ui/scenarios/snapshot_only.toml");
    let paths = prepare_run_paths("snapshot_only_scenario");

    let output = run_scenario(&binary, &root, &fixture_workspace, &scenario, &paths);

    assert!(
        output.status.success(),
        "snapshot-only run should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    require_file(&paths.state_json);

    let snapshot = read_snapshot(&paths.state_json);
    assert_eq!(snapshot.outcome, "completed");
    assert_eq!(snapshot.scenario_name, "snapshot-only");
    assert!(snapshot.progress.done);
    assert_eq!(
        snapshot.progress.current_step,
        snapshot.progress.total_steps
    );

    let selected = snapshot
        .selected_request
        .expect("snapshot should include selected request");
    assert_eq!(selected.index, 0);
}
