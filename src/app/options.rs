use base64::{Engine, engine::general_purpose};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestMode {
    Rest,
    GraphQl,
}

impl RequestMode {
    pub const ALL: [Self; 2] = [Self::Rest, Self::GraphQl];
}

impl std::fmt::Display for RequestMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rest => f.write_str("REST"),
            Self::GraphQl => f.write_str("GraphQL"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    None,
    Bearer,
    ApiKey,
    Basic,
}

impl AuthKind {
    pub const ALL: [Self; 4] = [Self::None, Self::Bearer, Self::ApiKey, Self::Basic];
}

impl std::fmt::Display for AuthKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Bearer => f.write_str("Bearer token"),
            Self::ApiKey => f.write_str("API key"),
            Self::Basic => f.write_str("Basic auth"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthState {
    pub kind: AuthKind,
    pub bearer_token: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub basic_username: String,
    pub basic_password: String,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            kind: AuthKind::None,
            bearer_token: String::new(),
            api_key_name: "Authorization".to_string(),
            api_key_value: String::new(),
            basic_username: String::new(),
            basic_password: String::new(),
        }
    }
}

pub fn build_graphql_body(query: &str, variables: &str) -> String {
    let variables_json: serde_json::Value =
        serde_json::from_str(variables).unwrap_or_else(|_| json!({}));
    json!({
        "query": query,
        "variables": variables_json,
    })
    .to_string()
}

pub fn apply_auth_headers(existing: &str, auth: &AuthState) -> String {
    match auth.kind {
        AuthKind::None => existing.to_string(),
        AuthKind::Bearer => {
            let mut out = existing.to_string();
            out.push_str("\nAuthorization: Bearer ");
            out.push_str(auth.bearer_token.trim());
            out
        }
        AuthKind::ApiKey => {
            let mut out = existing.to_string();
            out.push('\n');
            out.push_str(auth.api_key_name.trim());
            out.push_str(": ");
            out.push_str(auth.api_key_value.trim());
            out
        }
        AuthKind::Basic => {
            let token = general_purpose::STANDARD
                .encode(format!("{}:{}", auth.basic_username, auth.basic_password));
            let mut out = existing.to_string();
            out.push_str("\nAuthorization: Basic ");
            out.push_str(&token);
            out
        }
    }
}
