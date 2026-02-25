use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::{Subscription, Task, time, window};
use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::launch::AutomationOptions;
use crate::model::RequestId;

use super::{Message, Zagel};

const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 20_000;

#[derive(Debug, Clone)]
pub(super) struct AutomationRuntime {
    scenario_name: String,
    steps: Vec<ScenarioStep>,
    current_step: usize,
    pending_wait: Option<PendingWait>,
    pending_screenshot_name: Option<String>,
    screenshot_dir: PathBuf,
    state_output_path: Option<PathBuf>,
    window_id: Option<window::Id>,
    exit_when_done: bool,
    done: bool,
}

impl AutomationRuntime {
    pub(super) fn load(options: AutomationOptions) -> Result<Self, String> {
        let scenario_path = options.scenario_path;
        let raw = fs::read_to_string(&scenario_path)
            .map_err(|err| format!("failed to read scenario {}: {err}", scenario_path.display()))?;
        let parsed: ScenarioFile = toml::from_str(&raw).map_err(|err| {
            format!(
                "failed to parse scenario {}: {err}",
                scenario_path.display()
            )
        })?;

        let mut all_steps = parsed.step;
        all_steps.extend(parsed.steps);
        if all_steps.is_empty() {
            return Err(format!(
                "scenario {} has no [[step]] entries",
                scenario_path.display()
            ));
        }

        let mut steps = Vec::with_capacity(all_steps.len());
        for (index, raw_step) in all_steps.iter().enumerate() {
            steps.push(ScenarioStep::from_raw(raw_step).map_err(|err| {
                format!(
                    "invalid scenario step #{index} in {}: {err}",
                    scenario_path.display()
                )
            })?);
        }

        fs::create_dir_all(&options.screenshot_dir).map_err(|err| {
            format!(
                "failed to create screenshot directory {}: {err}",
                options.screenshot_dir.display()
            )
        })?;

        if let Some(state_path) = options.state_output_path.as_ref()
            && let Some(parent) = state_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create state output directory {}: {err}",
                    parent.display()
                )
            })?;
        }

        let scenario_name = parsed.name.unwrap_or_else(|| {
            scenario_path
                .file_stem()
                .and_then(OsStr::to_str)
                .map_or_else(|| "scenario".to_string(), str::to_owned)
        });

        Ok(Self {
            scenario_name,
            steps,
            current_step: 0,
            pending_wait: None,
            pending_screenshot_name: None,
            screenshot_dir: options.screenshot_dir,
            state_output_path: options.state_output_path,
            window_id: None,
            exit_when_done: options.exit_when_done,
            done: false,
        })
    }

    const fn should_poll(&self) -> bool {
        self.pending_wait.is_some() && !self.done
    }
}

#[derive(Debug, Clone)]
enum ScenarioStep {
    SelectRequest {
        selector: RequestSelector,
        timeout: Duration,
    },
    Send,
    WaitForStatus {
        status: u16,
        timeout: Duration,
    },
    WaitForText {
        text: String,
        timeout: Duration,
    },
    WaitForMillis(Duration),
    Screenshot {
        name: String,
    },
}

impl ScenarioStep {
    fn from_raw(raw: &RawStep) -> Result<Self, String> {
        let timeout = Duration::from_millis(raw.timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS));
        match raw.action.trim().to_ascii_lowercase().as_str() {
            "select_request" => {
                let value = raw.required_string("select_request")?;
                let selector = RequestSelector::parse(value)?;
                Ok(Self::SelectRequest { selector, timeout })
            }
            "send" => Ok(Self::Send),
            "wait_for_status" => {
                let value = raw.required_string("wait_for_status")?;
                let status = value
                    .parse::<u16>()
                    .map_err(|_| format!("invalid status code '{value}'"))?;
                Ok(Self::WaitForStatus { status, timeout })
            }
            "wait_for_text" => {
                let text = raw.required_string("wait_for_text")?.to_string();
                Ok(Self::WaitForText { text, timeout })
            }
            "wait_for_millis" => {
                let millis = raw.required_u64("wait_for_millis")?;
                Ok(Self::WaitForMillis(Duration::from_millis(millis)))
            }
            "screenshot" => {
                let name = raw.required_string("screenshot")?.to_string();
                Ok(Self::Screenshot { name })
            }
            other => Err(format!("unsupported action '{other}'")),
        }
    }
}

#[derive(Debug, Clone)]
enum PendingWait {
    RequestAvailable {
        selector: RequestSelector,
        started: Instant,
        timeout: Duration,
    },
    ResponseStatus {
        status: u16,
        started: Instant,
        timeout: Duration,
    },
    TextPresent {
        text: String,
        started: Instant,
        timeout: Duration,
    },
    Delay {
        started: Instant,
        duration: Duration,
    },
}

#[derive(Debug, Clone)]
struct RequestSelector {
    path: PathBuf,
    index: usize,
}

impl RequestSelector {
    fn parse(raw: &str) -> Result<Self, String> {
        let Some((path, index)) = raw.rsplit_once('#') else {
            return Err(format!("request selector '{raw}' must be '<path>#<index>'"));
        };
        if path.trim().is_empty() {
            return Err("request selector path cannot be empty".to_string());
        }
        let index = index
            .parse::<usize>()
            .map_err(|_| format!("invalid request index '{index}'"))?;
        Ok(Self {
            path: PathBuf::from(path),
            index,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ScenarioFile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    step: Vec<RawStep>,
    #[serde(default)]
    steps: Vec<RawStep>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawStep {
    action: String,
    value: Option<StepValue>,
    timeout_ms: Option<u64>,
}

impl RawStep {
    fn required_string(&self, action: &str) -> Result<&str, String> {
        match self.value.as_ref() {
            Some(StepValue::Text(text)) if !text.trim().is_empty() => Ok(text),
            Some(StepValue::Integer(number)) => Err(format!(
                "action '{action}' expects a string value, got {number}"
            )),
            Some(StepValue::Text(_)) | None => {
                Err(format!("action '{action}' requires a non-empty value"))
            }
        }
    }

    fn required_u64(&self, action: &str) -> Result<u64, String> {
        match self.value.as_ref() {
            Some(StepValue::Integer(number)) => Ok(*number),
            Some(StepValue::Text(text)) => text
                .parse::<u64>()
                .map_err(|_| format!("action '{action}' value '{text}' is not a valid number")),
            None => Err(format!("action '{action}' requires a numeric value")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum StepValue {
    Text(String),
    Integer(u64),
}

#[derive(Debug, Clone)]
enum SnapshotOutcome {
    Completed,
    Failed(String),
}

#[derive(Debug, Serialize)]
struct AutomationStateSnapshot {
    scenario_name: String,
    outcome: String,
    failure_reason: Option<String>,
    progress: SnapshotProgress,
    status_line: String,
    selected_request: Option<SelectedRequestSnapshot>,
    request_mode: String,
    response_display: String,
    response_tab: String,
    active_environment: Option<String>,
    project_roots: Vec<String>,
    global_env_roots: Vec<String>,
    environments: Vec<EnvironmentSnapshot>,
    draft: RequestDraftSnapshot,
    graphql_query: String,
    graphql_variables: String,
    header_rows: Vec<HeaderRowSnapshot>,
    response_viewer: String,
    response: Option<ResponseSnapshot>,
    collections: Vec<HttpFileSnapshot>,
}

#[derive(Debug, Serialize)]
struct SnapshotProgress {
    current_step: usize,
    total_steps: usize,
    done: bool,
}

#[derive(Debug, Serialize)]
struct SelectedRequestSnapshot {
    path: String,
    index: usize,
}

#[derive(Debug, Serialize)]
struct RequestDraftSnapshot {
    title: String,
    method: String,
    url: String,
    headers: String,
    body: String,
}

#[derive(Debug, Serialize)]
struct HeaderRowSnapshot {
    name: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct EnvironmentSnapshot {
    name: String,
    scope: String,
    vars: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ResponseSnapshot {
    status: Option<u16>,
    duration_ms: Option<u128>,
    error: Option<String>,
    headers: Vec<(String, String)>,
    body_raw: String,
    body_pretty: Option<String>,
}

#[derive(Debug, Serialize)]
struct HttpFileSnapshot {
    path: String,
    requests: Vec<RequestDraftSnapshot>,
}

fn sanitize_screenshot_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "shot".to_string()
    } else {
        trimmed.to_string()
    }
}

fn immediate(message: Message) -> Task<Message> {
    Task::perform(async move { message }, |message| message)
}

fn save_png(path: &Path, screenshot: window::Screenshot) -> Result<(), String> {
    let window::Screenshot { rgba, size, .. } = screenshot;
    let Some(image) = RgbaImage::from_raw(size.width, size.height, rgba.to_vec()) else {
        return Err("screenshot buffer has unexpected size".to_string());
    };
    image
        .save(path)
        .map_err(|err| format!("failed to save {}: {err}", path.display()))
}

fn snapshot_request_draft(draft: &crate::model::RequestDraft) -> RequestDraftSnapshot {
    RequestDraftSnapshot {
        title: draft.title.clone(),
        method: draft.method.as_str().to_string(),
        url: draft.url.clone(),
        headers: draft.headers.clone(),
        body: draft.body.clone(),
    }
}

const fn environment_scope_label(scope: &crate::model::EnvironmentScope) -> &'static str {
    match scope {
        crate::model::EnvironmentScope::Project(_) => "project",
        crate::model::EnvironmentScope::Global => "global",
        crate::model::EnvironmentScope::Default => "default",
    }
}

fn outcome_details(outcome: &SnapshotOutcome) -> (&'static str, Option<String>) {
    match outcome {
        SnapshotOutcome::Completed => ("completed", None),
        SnapshotOutcome::Failed(reason) => ("failed", Some(reason.clone())),
    }
}

impl Zagel {
    pub(super) fn automation_subscription(&self) -> Option<Subscription<Message>> {
        self.automation.as_ref().and_then(|runtime| {
            if runtime.should_poll() {
                Some(time::every(WAIT_POLL_INTERVAL).map(|_| Message::AutomationPoll))
            } else {
                None
            }
        })
    }

    pub(super) fn automation_start_task(&self) -> Task<Message> {
        if self.automation.is_some() {
            immediate(Message::AutomationStart)
        } else {
            Task::none()
        }
    }

    pub(super) fn automation_pulse_task(&self) -> Task<Message> {
        self.automation
            .as_ref()
            .filter(|runtime| !runtime.done)
            .map_or_else(Task::none, |_| immediate(Message::AutomationPoll))
    }

    pub(super) fn handle_automation_pulse(&mut self) -> Task<Message> {
        let Some(mut runtime) = self.automation.take() else {
            return Task::none();
        };

        let task = self.drive_automation(&mut runtime);
        self.automation = Some(runtime);
        task
    }

    pub(super) fn handle_automation_window_resolved(
        &mut self,
        window_id: Option<window::Id>,
    ) -> Task<Message> {
        let Some(mut runtime) = self.automation.take() else {
            return Task::none();
        };

        if let Some(window_id) = window_id {
            runtime.window_id = Some(window_id);
        }

        let task = if runtime.done && runtime.exit_when_done {
            runtime
                .window_id
                .map_or_else(Task::none, window::close::<Message>)
        } else {
            self.drive_automation(&mut runtime)
        };
        self.automation = Some(runtime);
        task
    }

    pub(super) fn handle_automation_screenshot(
        &mut self,
        screenshot: window::Screenshot,
    ) -> Task<Message> {
        let Some(mut runtime) = self.automation.take() else {
            return Task::none();
        };

        let task = if let Some(name) = runtime.pending_screenshot_name.take() {
            let stem = sanitize_screenshot_name(&name);
            let path = runtime
                .screenshot_dir
                .join(format!("{:02}-{stem}.png", runtime.current_step + 1));
            match save_png(&path, screenshot) {
                Ok(()) => {
                    runtime.current_step += 1;
                    self.update_status_with_missing(&format!(
                        "Automation screenshot saved: {}",
                        path.display()
                    ));
                    self.drive_automation(&mut runtime)
                }
                Err(err) => self.fail_automation(&mut runtime, &err),
            }
        } else {
            self.fail_automation(
                &mut runtime,
                "received a screenshot but no screenshot step is pending",
            )
        };

        self.automation = Some(runtime);
        task
    }

    fn write_automation_state_snapshot(
        &self,
        runtime: &AutomationRuntime,
        outcome: &SnapshotOutcome,
    ) -> Result<Option<PathBuf>, String> {
        let Some(path) = runtime.state_output_path.as_ref() else {
            return Ok(None);
        };

        let snapshot = self.build_automation_state_snapshot(runtime, outcome);
        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|err| format!("failed to serialize automation state snapshot: {err}"))?;
        fs::write(path, json)
            .map_err(|err| format!("failed to write automation state {}: {err}", path.display()))?;
        Ok(Some(path.clone()))
    }

    fn build_automation_state_snapshot(
        &self,
        runtime: &AutomationRuntime,
        outcome: &SnapshotOutcome,
    ) -> AutomationStateSnapshot {
        let (outcome, failure_reason) = outcome_details(outcome);
        let selected_request = self.workspace.selection().map(|selection| match selection {
            RequestId::HttpFile { path, index } => SelectedRequestSnapshot {
                path: path.display().to_string(),
                index: *index,
            },
        });
        let active_environment = self
            .environments
            .get(self.active_environment)
            .map(|environment| environment.name.clone());
        let environments = self
            .environments
            .iter()
            .map(|environment| EnvironmentSnapshot {
                name: environment.name.clone(),
                scope: environment_scope_label(&environment.scope).to_string(),
                vars: environment.vars.clone(),
            })
            .collect();
        let response = self.response.as_ref().map(|response| ResponseSnapshot {
            status: response.preview.status,
            duration_ms: response
                .preview
                .duration
                .map(|duration| duration.as_millis()),
            error: response.preview.error.clone(),
            headers: response.preview.headers.clone(),
            body_raw: response.body.raw().to_string(),
            body_pretty: response.body.pretty_text().map(str::to_owned),
        });

        let mut ordered_paths = self.workspace.http_file_order().clone();
        let mut additional_paths = self
            .workspace
            .http_files()
            .keys()
            .filter(|path| !ordered_paths.contains(path))
            .cloned()
            .collect::<Vec<_>>();
        additional_paths
            .sort_by(|left, right| left.to_string_lossy().cmp(&right.to_string_lossy()));
        ordered_paths.extend(additional_paths);

        let collections = ordered_paths
            .into_iter()
            .filter_map(|path| {
                self.workspace
                    .http_files()
                    .get(&path)
                    .map(|file| HttpFileSnapshot {
                        path: file.path.display().to_string(),
                        requests: file.requests.iter().map(snapshot_request_draft).collect(),
                    })
            })
            .collect();

        AutomationStateSnapshot {
            scenario_name: runtime.scenario_name.clone(),
            outcome: outcome.to_string(),
            failure_reason,
            progress: SnapshotProgress {
                current_step: runtime.current_step,
                total_steps: runtime.steps.len(),
                done: runtime.done,
            },
            status_line: self.status_line.clone(),
            selected_request,
            request_mode: self.mode.to_string(),
            response_display: self.response_display.to_string(),
            response_tab: self.response_tab.to_string(),
            active_environment,
            project_roots: self
                .project_roots()
                .iter()
                .map(|root| root.as_path().display().to_string())
                .collect(),
            global_env_roots: self
                .global_env_roots()
                .iter()
                .map(|root| root.as_path().display().to_string())
                .collect(),
            environments,
            draft: snapshot_request_draft(&self.draft),
            graphql_query: self.graphql_query.text(),
            graphql_variables: self.graphql_variables.text(),
            header_rows: self
                .header_rows
                .iter()
                .map(|row| HeaderRowSnapshot {
                    name: row.name.clone(),
                    value: row.value.clone(),
                })
                .collect(),
            response_viewer: self.response_viewer.text(),
            response,
            collections,
        }
    }

    fn wait_satisfied(&self, wait: &PendingWait) -> bool {
        match wait {
            PendingWait::RequestAvailable { selector, .. } => {
                self.resolve_request_selector(selector).is_some()
            }
            PendingWait::ResponseStatus { status, .. } => self
                .response
                .as_ref()
                .and_then(|response| response.preview.status)
                .is_some_and(|actual| actual == *status),
            PendingWait::TextPresent { text, .. } => {
                self.status_line.contains(text)
                    || self
                        .response
                        .as_ref()
                        .is_some_and(|response| response.body.raw().contains(text))
                    || self.response_viewer.text().contains(text)
            }
            PendingWait::Delay { started, duration } => started.elapsed() >= *duration,
        }
    }

    fn wait_timeout_message(wait: &PendingWait) -> Option<String> {
        match wait {
            PendingWait::RequestAvailable {
                selector,
                started,
                timeout,
            } if started.elapsed() > *timeout => Some(format!(
                "timed out waiting for request {}#{}",
                selector.path.display(),
                selector.index
            )),
            PendingWait::ResponseStatus {
                status,
                started,
                timeout,
            } if started.elapsed() > *timeout => {
                Some(format!("timed out waiting for HTTP status {status}"))
            }
            PendingWait::TextPresent {
                text,
                started,
                timeout,
            } if started.elapsed() > *timeout => {
                Some(format!("timed out waiting for text '{text}'"))
            }
            PendingWait::Delay { .. }
            | PendingWait::RequestAvailable { .. }
            | PendingWait::ResponseStatus { .. }
            | PendingWait::TextPresent { .. } => None,
        }
    }

    fn resolve_request_selector(&self, selector: &RequestSelector) -> Option<RequestId> {
        let mut candidates = self
            .workspace
            .http_files()
            .iter()
            .filter(|(path, _)| {
                if selector.path.is_absolute() {
                    *path == &selector.path
                } else {
                    path.ends_with(&selector.path)
                        || self
                            .project_root_for_path(path)
                            .and_then(|root| path.strip_prefix(root.as_path()).ok())
                            .is_some_and(|relative| relative == selector.path)
                }
            })
            .collect::<Vec<_>>();

        candidates
            .sort_by(|(left, _), (right, _)| left.to_string_lossy().cmp(&right.to_string_lossy()));

        candidates.into_iter().find_map(|(path, file)| {
            (selector.index < file.requests.len()).then(|| RequestId::HttpFile {
                path: path.clone(),
                index: selector.index,
            })
        })
    }

    fn complete_automation(&mut self, runtime: &mut AutomationRuntime) -> Task<Message> {
        runtime.done = true;
        self.update_status_with_missing(&format!(
            "Automation '{}' completed",
            runtime.scenario_name
        ));
        let state_path =
            match self.write_automation_state_snapshot(runtime, &SnapshotOutcome::Completed) {
                Ok(path) => path,
                Err(err) => {
                    eprintln!("automation: {err}");
                    None
                }
            };
        if let Some(path) = state_path {
            self.update_status_with_missing(&format!(
                "Automation '{}' completed (state: {})",
                runtime.scenario_name,
                path.display()
            ));
        }
        if runtime.exit_when_done {
            runtime.window_id.map_or_else(
                || window::latest().map(Message::AutomationWindowResolved),
                window::close::<Message>,
            )
        } else {
            Task::none()
        }
    }

    fn fail_automation(&mut self, runtime: &mut AutomationRuntime, reason: &str) -> Task<Message> {
        runtime.done = true;
        self.update_status_with_missing(&format!("Automation failed: {reason}"));
        let state_path = match self
            .write_automation_state_snapshot(runtime, &SnapshotOutcome::Failed(reason.to_string()))
        {
            Ok(path) => path,
            Err(err) => {
                eprintln!("automation: {err}");
                None
            }
        };
        if let Some(path) = state_path {
            self.update_status_with_missing(&format!(
                "Automation failed: {reason} (state: {})",
                path.display()
            ));
        }
        eprintln!("automation failed: {reason}");
        if runtime.exit_when_done {
            runtime.window_id.map_or_else(
                || window::latest().map(Message::AutomationWindowResolved),
                window::close::<Message>,
            )
        } else {
            Task::none()
        }
    }

    fn drive_automation(&mut self, runtime: &mut AutomationRuntime) -> Task<Message> {
        if runtime.done {
            return Task::none();
        }

        if let Some(wait) = runtime.pending_wait.as_ref() {
            if self.wait_satisfied(wait) {
                runtime.pending_wait = None;
                runtime.current_step += 1;
            } else if let Some(timeout_message) = Self::wait_timeout_message(wait) {
                return self.fail_automation(runtime, &timeout_message);
            } else {
                return Task::none();
            }
        }

        loop {
            let Some(step) = runtime.steps.get(runtime.current_step).cloned() else {
                return self.complete_automation(runtime);
            };
            match step {
                ScenarioStep::SelectRequest { selector, timeout } => {
                    if let Some(id) = self.resolve_request_selector(&selector) {
                        self.apply_selection(&id);
                        runtime.current_step += 1;
                        continue;
                    }
                    runtime.pending_wait = Some(PendingWait::RequestAvailable {
                        selector,
                        started: Instant::now(),
                        timeout,
                    });
                    return Task::none();
                }
                ScenarioStep::Send => {
                    runtime.current_step += 1;
                    return immediate(Message::Send);
                }
                ScenarioStep::WaitForStatus { status, timeout } => {
                    let wait = PendingWait::ResponseStatus {
                        status,
                        started: Instant::now(),
                        timeout,
                    };
                    if self.wait_satisfied(&wait) {
                        runtime.current_step += 1;
                        continue;
                    }
                    runtime.pending_wait = Some(wait);
                    return Task::none();
                }
                ScenarioStep::WaitForText { text, timeout } => {
                    let wait = PendingWait::TextPresent {
                        text,
                        started: Instant::now(),
                        timeout,
                    };
                    if self.wait_satisfied(&wait) {
                        runtime.current_step += 1;
                        continue;
                    }
                    runtime.pending_wait = Some(wait);
                    return Task::none();
                }
                ScenarioStep::WaitForMillis(duration) => {
                    if duration.is_zero() {
                        runtime.current_step += 1;
                        continue;
                    }
                    runtime.pending_wait = Some(PendingWait::Delay {
                        started: Instant::now(),
                        duration,
                    });
                    return Task::none();
                }
                ScenarioStep::Screenshot { name } => {
                    let Some(window_id) = runtime.window_id else {
                        return window::latest().map(Message::AutomationWindowResolved);
                    };
                    runtime.pending_screenshot_name = Some(name);
                    return window::screenshot(window_id)
                        .map(Message::AutomationScreenshotCaptured);
                }
            }
        }
    }
}
