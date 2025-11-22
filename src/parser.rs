use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use walkdir::WalkDir;

use crate::model::{HttpFile, Method, RequestDraft};

pub async fn scan_http_files(root: PathBuf, max_depth: usize) -> HashMap<PathBuf, HttpFile> {
    let mut files = HashMap::new();
    for entry in WalkDir::new(root)
        .follow_links(true)
        .max_depth(max_depth)
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
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
