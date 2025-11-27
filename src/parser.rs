use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use walkdir::WalkDir;

use crate::model::{Environment, HttpFile, Method, RequestDraft, RequestId};

pub async fn scan_http_files(root: PathBuf, max_depth: usize) -> HashMap<PathBuf, HttpFile> {
    let mut files = HashMap::new();
    for entry in WalkDir::new(root).follow_links(true).max_depth(max_depth) {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("http") {
            continue;
        }

        if let Ok(file) = parse_http_file(entry.path()) {
            files.insert(entry.into_path(), file);
        }
    }
    files
}

pub async fn scan_env_files(root: PathBuf, max_depth: usize) -> Vec<Environment> {
    let mut envs = Vec::new();
    for entry in WalkDir::new(root).follow_links(true).max_depth(max_depth) {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_file() {
            continue;
        }

        if !is_env_file(entry.path()) {
            continue;
        }

        if let Ok(env) = parse_env_file(entry.path()) {
            envs.push(env);
        }
    }
    envs.sort_by(|a, b| a.name.cmp(&b.name));
    envs
}

pub fn parse_http_file(path: &Path) -> anyhow::Result<HttpFile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mut blocks = Vec::new();
    let mut current = Vec::new();

    for line in content.lines() {
        if line.trim_start().starts_with("###") {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            current.push(line.to_string());
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }

    let mut requests = Vec::new();
    for (idx, block) in blocks.into_iter().enumerate() {
        if let Some(req) = parse_request_block(&block) {
            requests.push(req);
        } else {
            requests.push(RequestDraft {
                title: format!("Untitled {}", idx + 1),
                ..Default::default()
            });
        }
    }

    Ok(HttpFile {
        path: path.to_path_buf(),
        requests,
    })
}

pub async fn persist_request(
    http_root: PathBuf,
    selection: Option<RequestId>,
    draft: RequestDraft,
    explicit_path: Option<PathBuf>,
) -> anyhow::Result<(PathBuf, usize)> {
    let (path, replace_index) = match (selection, explicit_path) {
        (Some(RequestId::HttpFile { path, index }), None) => (path, Some(index)),
        (_, Some(path)) => (path, None),
        _ => (suggest_http_path(&http_root, &draft.title), None),
    };

    let mut requests = if path.exists() {
        parse_http_file(&path)?.requests
    } else {
        Vec::new()
    };

    let index = if let Some(idx) = replace_index {
        if idx < requests.len() {
            requests[idx] = draft;
            idx
        } else {
            requests.push(draft);
            requests.len() - 1
        }
    } else {
        requests.push(draft);
        requests.len() - 1
    };

    write_http_file(&path, &requests)?;
    Ok((path, index))
}

pub fn write_http_file(path: &Path, requests: &[RequestDraft]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let mut content = String::new();
    for (idx, req) in requests.iter().enumerate() {
        if idx > 0 {
            content.push('\n');
        }
        writeln!(content, "### {}", req.title).ok();
        content.push_str(&format_request_block(req));
    }

    fs::write(path, content)
        .with_context(|| format!("Failed to write requests to {}", path.display()))
}

fn format_request_block(req: &RequestDraft) -> String {
    let mut block = String::new();
    writeln!(block, "{} {}", req.method.as_str(), req.url).ok();
    let headers = req.headers.trim_end();
    if !headers.is_empty() {
        block.push_str(headers);
        block.push('\n');
    }
    block.push('\n');
    if !req.body.is_empty() {
        block.push_str(&req.body);
        block.push('\n');
    }
    block
}

pub fn suggest_http_path(root: &Path, title: &str) -> PathBuf {
    let mut slug = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>();

    slug = slug
        .trim_matches('-')
        .trim_matches('_')
        .trim()
        .trim_end_matches('-')
        .trim_end_matches('_')
        .to_string();

    if slug.is_empty() {
        slug = "request".to_string();
    }

    root.join(format!("{slug}.http"))
}

fn parse_request_block(lines: &[String]) -> Option<RequestDraft> {
    let mut lines_iter = lines.iter().skip_while(|l| l.trim().is_empty());
    let first = lines_iter.next()?;
    let mut parts = first.trim().splitn(2, ' ');
    let method = parts.next()?;
    let url = parts.next().unwrap_or_default().to_string();

    let mut headers = Vec::new();
    let mut body = Vec::new();
    let mut in_headers = true;

    for line in lines_iter {
        if in_headers {
            if line.trim().is_empty() {
                in_headers = false;
                continue;
            }
            headers.push(line.clone());
        } else {
            body.push(line.clone());
        }
    }

    Some(RequestDraft {
        title: url.clone(),
        method: Method::from(method),
        url,
        headers: headers.join("\n"),
        body: body.join("\n"),
    })
}

fn parse_env_file(path: &Path) -> anyhow::Result<Environment> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mut vars = BTreeMap::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            vars.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let name = path
        .file_stem()
        .or_else(|| path.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("environment")
        .to_string();

    Ok(Environment { name, vars })
}

fn is_env_file(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if file_name.starts_with(".env") {
        return true;
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("env"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use tempfile::tempdir;

    #[test]
    fn suggest_http_path_slugifies_title() {
        let root = PathBuf::from("/tmp");
        let path = suggest_http_path(&root, "My First Request!");
        assert_eq!(path, root.join("my-first-request.http"));
    }

    #[test]
    fn persist_request_creates_file_for_new_request() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let target = root.join("new.http");
        let draft = RequestDraft {
            title: "New".into(),
            method: Method::Post,
            url: "https://example.com".into(),
            headers: "Content-Type: application/json".into(),
            body: "{\"ok\":true}".into(),
        };

        let (path, idx) = block_on(persist_request(
            root,
            None,
            draft.clone(),
            Some(target.clone()),
        ))
        .expect("persist request");

        assert_eq!(path, target);
        assert_eq!(idx, 0);

        let parsed = parse_http_file(&path).expect("parse saved file");
        assert_eq!(parsed.requests.len(), 1);
        let saved = &parsed.requests[0];
        assert_eq!(saved.title, draft.url);
        assert_eq!(saved.method, draft.method);
        assert_eq!(saved.url, draft.url);
        assert_eq!(saved.headers.trim(), draft.headers.trim());
        assert_eq!(saved.body.trim(), draft.body.trim());
    }

    #[test]
    fn persist_request_replaces_existing_index() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let path = root.join("existing.http");

        let original = RequestDraft {
            title: "Original".into(),
            method: Method::Get,
            url: "https://example.com/old".into(),
            headers: String::new(),
            body: String::new(),
        };
        write_http_file(&path, &[original]).expect("write original");

        let updated = RequestDraft {
            title: "Updated".into(),
            method: Method::Delete,
            url: "https://example.com/new".into(),
            headers: "Authorization: test".into(),
            body: "hi".into(),
        };

        let selection = Some(RequestId::HttpFile {
            path: path.clone(),
            index: 0,
        });

        let (_path, idx) = block_on(persist_request(root, selection, updated.clone(), None))
            .expect("persist update");

        assert_eq!(idx, 0);
        let parsed = parse_http_file(&path).expect("parse updated");
        assert_eq!(parsed.requests.len(), 1);
        let saved = &parsed.requests[0];
        assert_eq!(saved.title, updated.url);
        assert_eq!(saved.method, updated.method);
        assert_eq!(saved.url, updated.url);
        assert_eq!(saved.headers.trim(), updated.headers.trim());
        assert_eq!(saved.body.trim(), updated.body.trim());
    }
}
