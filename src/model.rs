use std::collections::BTreeMap;
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
    pub const ALL: [Method; 6] = [
        Method::Get,
        Method::Post,
        Method::Put,
        Method::Delete,
        Method::Patch,
        Method::Head,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
            Method::Head => "HEAD",
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
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "PATCH" => Method::Patch,
            "HEAD" => Method::Head,
            _ => Method::Get,
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
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub name: String,
    pub requests: Vec<RequestDraft>,
}

#[derive(Debug, Clone)]
pub struct HttpFile {
    pub path: PathBuf,
    pub requests: Vec<RequestDraft>,
}

#[derive(Debug, Clone)]
pub struct UnsavedTab {
    pub id: u32,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequestId {
    Collection { collection: usize, index: usize },
    HttpFile { path: PathBuf, index: usize },
    Unsaved(u32),
}

#[derive(Debug, Clone)]
pub struct ResponsePreview {
    pub status: Option<u16>,
    pub duration: Option<Duration>,
    pub body: Option<String>,
    pub error: Option<String>,
}

impl ResponsePreview {
    pub fn error(message: String) -> Self {
        Self {
            status: None,
            duration: None,
            body: None,
            error: Some(message),
        }
    }
}

pub fn apply_environment(input: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = input.to_string();
    for (key, value) in vars {
        let needle = format!("{{{{{}}}}}", key);
        out = out.replace(&needle, value);
    }
    out
}
