use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
}

impl Method {
    pub const ALL: [Self; 6] = [
        Self::Get,
        Self::Post,
        Self::Put,
        Self::Delete,
        Self::Patch,
        Self::Head,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
            Self::Head => "HEAD",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Method {
    fn from(value: &str) -> Self {
        match value.to_uppercase().as_str() {
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "PATCH" => Self::Patch,
            "HEAD" => Self::Head,
            _ => Self::Get,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestDraft {
    pub title: String,
    pub method: Method,
    pub url: String,
    pub headers: String,
    pub body: String,
}

impl Default for RequestDraft {
    fn default() -> Self {
        Self {
            title: "Untitled request".to_string(),
            method: Method::Get,
            url: String::from("https://example.com"),
            headers: String::new(),
            body: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub name: String,
    pub vars: BTreeMap<String, String>,
    pub scope: EnvironmentScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnvironmentScope {
    Project(PathBuf),
    Global,
    Default,
}

impl Environment {
    pub fn visible_for_project(&self, project_root: Option<&Path>) -> bool {
        match &self.scope {
            EnvironmentScope::Project(root) => project_root.is_some_and(|project| project == root),
            EnvironmentScope::Global | EnvironmentScope::Default => true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpFile {
    pub path: PathBuf,
    pub requests: Vec<RequestDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequestId {
    HttpFile { path: PathBuf, index: usize },
}

#[derive(Debug, Clone)]
pub struct ResponsePreview {
    pub status: Option<u16>,
    pub duration: Option<Duration>,
    pub body: Option<String>,
    pub headers: Vec<(String, String)>,
    pub error: Option<String>,
}

impl ResponsePreview {
    pub const fn error(message: String) -> Self {
        Self {
            status: None,
            duration: None,
            body: None,
            headers: Vec::new(),
            error: Some(message),
        }
    }
}

pub fn apply_environment(input: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = input.to_string();
    for (key, value) in vars {
        let needle = format!("{{{{{key}}}}}");
        out = out.replace(&needle, value);
    }
    out
}
