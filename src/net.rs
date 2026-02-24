use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use base64::{Engine, engine::general_purpose};
use reqwest::Client;
use serde::Deserialize;

use crate::app::{
    AuthState, ClientSecretMethod, OAuth2ClientCredentialsAuthState, apply_auth_headers,
};
use crate::model::{Environment, RequestDraft, ResponsePreview, apply_environment};

const OAUTH2_TOKEN_EXPIRY_SKEW: Duration = Duration::from_secs(30);
const OAUTH2_TOKEN_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct OAuth2TokenCacheEntry {
    key: OAuth2TokenCacheKey,
    token: OAuth2AccessToken,
}

#[derive(Clone, PartialEq, Eq)]
struct OAuth2TokenCacheKey {
    token_url: String,
    client_id: String,
    client_secret: String,
    scope: String,
    client_secret_method: ClientSecretMethod,
    environment_name: Option<String>,
}

#[derive(Clone)]
struct OAuth2AccessToken {
    value: String,
    expires_at: Option<Instant>,
}

impl std::fmt::Debug for OAuth2TokenCacheEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuth2TokenCacheEntry")
            .field("key", &self.key)
            .field("token", &self.token)
            .finish()
    }
}

impl std::fmt::Debug for OAuth2TokenCacheKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuth2TokenCacheKey")
            .field("token_url", &self.token_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("scope", &self.scope)
            .field("client_secret_method", &self.client_secret_method)
            .field("environment_name", &self.environment_name)
            .finish()
    }
}

impl std::fmt::Debug for OAuth2AccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuth2AccessToken")
            .field("value", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl OAuth2AccessToken {
    fn is_still_valid(&self, now: Instant) -> bool {
        self.expires_at.is_none_or(|expires_at| {
            expires_at.saturating_duration_since(now) > OAUTH2_TOKEN_EXPIRY_SKEW
        })
    }
}

#[derive(Debug, Clone)]
pub struct SendOutcome {
    pub response: ResponsePreview,
    pub oauth2_cache: Option<OAuth2TokenCacheEntry>,
}

#[derive(Debug)]
struct Unresolved;

#[derive(Debug)]
struct Resolved;

#[derive(Debug)]
struct ClientCredentials<State> {
    token_url: String,
    client_id: String,
    client_secret: String,
    scope: String,
    client_secret_method: ClientSecretMethod,
    marker: PhantomData<State>,
}

impl ClientCredentials<Unresolved> {
    fn parse(auth: &OAuth2ClientCredentialsAuthState, env_vars: &BTreeMap<String, String>) -> Self {
        Self {
            token_url: apply_environment(&auth.token_url, env_vars),
            client_id: apply_environment(&auth.client_id, env_vars),
            client_secret: apply_environment(&auth.client_secret, env_vars),
            scope: apply_environment(&auth.scope, env_vars),
            client_secret_method: auth.client_secret_method,
            marker: PhantomData,
        }
    }

    fn resolve(self) -> Result<ClientCredentials<Resolved>, String> {
        let token_url = parse_required("OAuth2 token URL", self.token_url)?;
        let client_id = parse_required("OAuth2 client ID", self.client_id)?;
        let client_secret = parse_required("OAuth2 client secret", self.client_secret)?;

        Ok(ClientCredentials {
            token_url,
            client_id,
            client_secret,
            scope: self.scope.trim().to_string(),
            client_secret_method: self.client_secret_method,
            marker: PhantomData,
        })
    }
}

impl ClientCredentials<Resolved> {
    fn cache_key(&self, environment_name: Option<String>) -> OAuth2TokenCacheKey {
        OAuth2TokenCacheKey {
            token_url: self.token_url.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            scope: self.scope.clone(),
            client_secret_method: self.client_secret_method,
            environment_name,
        }
    }

    fn build_token_form(&self) -> Vec<(String, String)> {
        let mut form = vec![("grant_type".to_string(), "client_credentials".to_string())];
        if !self.scope.is_empty() {
            form.push(("scope".to_string(), self.scope.clone()));
        }
        if self.client_secret_method == ClientSecretMethod::RequestBody {
            form.push(("client_id".to_string(), self.client_id.clone()));
            form.push(("client_secret".to_string(), self.client_secret.clone()));
        }
        form
    }

    fn maybe_basic_authorization(&self) -> Option<String> {
        if self.client_secret_method != ClientSecretMethod::BasicAuth {
            return None;
        }
        let token =
            general_purpose::STANDARD.encode(format!("{}:{}", self.client_id, self.client_secret));
        Some(format!("Basic {token}"))
    }
}

#[derive(Debug, Deserialize)]
struct OAuth2TokenResponse {
    access_token: String,
    expires_in: Option<u64>,
}

pub async fn send_request(
    client: Client,
    mut draft: RequestDraft,
    env: Option<Environment>,
    auth: AuthState,
    oauth2_cache: Option<OAuth2TokenCacheEntry>,
) -> Result<SendOutcome, String> {
    let (env_name, env_vars) = env.map_or((None, BTreeMap::new()), |environment| {
        (Some(environment.name), environment.vars)
    });

    let (updated_cache, extra_authorization_header) =
        if let AuthState::OAuth2ClientCredentials(oauth) = auth {
            let (token, refreshed_cache) =
                resolve_oauth2_token(&client, &oauth, env_name.clone(), &env_vars, oauth2_cache)
                    .await?;
            (
                Some(refreshed_cache),
                Some(format!("Bearer {}", token.trim())),
            )
        } else {
            draft.headers = apply_auth_headers(&draft.headers, &auth);
            (None, None)
        };

    let response = send_request_with_resolved_environment(
        client,
        draft,
        env_name,
        env_vars,
        extra_authorization_header,
    )
    .await?;
    Ok(SendOutcome {
        response,
        oauth2_cache: updated_cache,
    })
}

fn parse_required(name: &str, value: String) -> Result<String, String> {
    let parsed = value.trim();
    if parsed.is_empty() {
        return Err(format!("{name} is required"));
    }
    if parsed.len() == value.len() {
        Ok(value)
    } else {
        Ok(parsed.to_string())
    }
}

async fn resolve_oauth2_token(
    client: &Client,
    auth: &OAuth2ClientCredentialsAuthState,
    env_name: Option<String>,
    env_vars: &BTreeMap<String, String>,
    cache: Option<OAuth2TokenCacheEntry>,
) -> Result<(String, OAuth2TokenCacheEntry), String> {
    let parsed = ClientCredentials::<Unresolved>::parse(auth, env_vars).resolve()?;
    let key = parsed.cache_key(env_name);

    if let Some(existing) = cache {
        if existing.key == key && existing.token.is_still_valid(Instant::now()) {
            return Ok((existing.token.value.clone(), existing));
        }
    }

    let token = fetch_oauth2_client_credentials_token(client, &parsed).await?;
    let entry = OAuth2TokenCacheEntry { key, token };
    Ok((entry.token.value.clone(), entry))
}

async fn fetch_oauth2_client_credentials_token(
    client: &Client,
    credentials: &ClientCredentials<Resolved>,
) -> Result<OAuth2AccessToken, String> {
    let mut request = client
        .post(&credentials.token_url)
        .form(&credentials.build_token_form())
        .timeout(OAUTH2_TOKEN_REQUEST_TIMEOUT);
    if let Some(auth_header) = credentials.maybe_basic_authorization() {
        request = request.header(reqwest::header::AUTHORIZATION, auth_header);
    }

    let response = request.send().await.map_err(|err| {
        format!(
            "OAuth2 token request failed ({}): {err}",
            credentials.token_url
        )
    })?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("Failed to read OAuth2 token response body: {err}"))?;

    if !status.is_success() {
        return Err(format!(
            "OAuth2 token request failed with HTTP {}: {body}",
            status.as_u16()
        ));
    }

    let parsed: OAuth2TokenResponse = serde_json::from_str(&body)
        .map_err(|err| format!("Invalid OAuth2 token response JSON: {err}"))?;
    let access_token = parse_required("OAuth2 access_token", parsed.access_token)?;
    let expires_at = parsed
        .expires_in
        .map(|seconds| {
            Instant::now()
                .checked_add(Duration::from_secs(seconds))
                .ok_or_else(|| format!("OAuth2 token expires_in overflow: {seconds}"))
        })
        .transpose()?;

    Ok(OAuth2AccessToken {
        value: access_token,
        expires_at,
    })
}

async fn send_request_with_resolved_environment(
    client: Client,
    draft: RequestDraft,
    env_name: Option<String>,
    env_vars: BTreeMap<String, String>,
    extra_authorization_header: Option<String>,
) -> Result<ResponsePreview, String> {
    let url = apply_environment(&draft.url, &env_vars);
    let headers_text = apply_environment(&draft.headers, &env_vars);
    let body_text = apply_environment(&draft.body, &env_vars);

    let mut log_lines = Vec::new();
    if let Some(name) = env_name.as_deref() {
        log_lines.push(format!("Environment: {name}"));
    }
    log_lines.push(format!("{} {url}", draft.method.as_str()));
    if !headers_text.trim().is_empty() {
        log_lines.push("Headers:".to_string());
        for line in headers_text.lines().filter(|line| !line.trim().is_empty()) {
            log_lines.push(format!("  {line}"));
        }
    }
    if !body_text.trim().is_empty() {
        log_lines.push("Body:".to_string());
        log_lines.push(body_text.clone());
    }
    if !log_lines.is_empty() {
        println!("{}", log_lines.join("\n"));
    }

    let mut request = client.request(
        reqwest::Method::from_bytes(draft.method.as_str().as_bytes())
            .unwrap_or(reqwest::Method::GET),
        url,
    );

    let has_extra_authorization_header = extra_authorization_header.is_some();
    for line in headers_text.lines() {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            if has_extra_authorization_header && name.eq_ignore_ascii_case("authorization") {
                continue;
            }
            request = request.header(name, value.trim());
        }
    }
    if let Some(value) = extra_authorization_header {
        request = request.header(reqwest::header::AUTHORIZATION, value);
    }

    let start = Instant::now();
    let response = request
        .body(body_text)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|header_value| (name.to_string(), header_value.to_string()))
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
