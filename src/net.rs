use std::time::Instant;

use reqwest::Client;

use crate::model::{Environment, RequestDraft, ResponsePreview, apply_environment};

pub async fn send_request(
    client: Client,
    draft: RequestDraft,
    env: Option<Environment>,
) -> Result<ResponsePreview, String> {
    let env_vars = env.map(|e| e.vars).unwrap_or_default();
    let url = apply_environment(&draft.url, &env_vars);
    let headers_text = apply_environment(&draft.headers, &env_vars);
    let body_text = apply_environment(&draft.body, &env_vars);

    let mut req = client.request(
        reqwest::Method::from_bytes(draft.method.as_str().as_bytes())
            .unwrap_or(reqwest::Method::GET),
        url,
    );

    for line in headers_text.lines() {
        if let Some((name, value)) = line.split_once(':') {
            req = req.header(name.trim(), value.trim());
        }
    }

    let start = Instant::now();
    let response = req
        .body(body_text)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.to_string(), v.to_string()))
        })
        .collect();
    let status = response.status().as_u16();
    let text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to read body".to_string());
    let duration = start.elapsed();

    Ok(ResponsePreview {
        status: Some(status),
        duration: Some(duration),
        body: Some(text),
        headers,
        error: None,
    })
}
