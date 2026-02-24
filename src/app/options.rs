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
    OAuth2ClientCredentials,
}

impl AuthKind {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Bearer,
        Self::ApiKey,
        Self::Basic,
        Self::OAuth2ClientCredentials,
    ];
}

impl std::fmt::Display for AuthKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Bearer => f.write_str("Bearer token"),
            Self::ApiKey => f.write_str("API key"),
            Self::Basic => f.write_str("Basic auth"),
            Self::OAuth2ClientCredentials => f.write_str("OAuth2 client credentials"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSecretMethod {
    BasicAuth,
    RequestBody,
}

impl ClientSecretMethod {
    pub const ALL: [Self; 2] = [Self::BasicAuth, Self::RequestBody];
}

impl std::fmt::Display for ClientSecretMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BasicAuth => f.write_str("client_secret_basic"),
            Self::RequestBody => f.write_str("client_secret_post"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BearerAuthState {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyAuthState {
    pub header_name: String,
    pub header_value: String,
}

impl Default for ApiKeyAuthState {
    fn default() -> Self {
        Self {
            header_name: "Authorization".to_string(),
            header_value: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BasicAuthState {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuth2ClientCredentialsAuthState {
    pub token_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scope: String,
    pub client_secret_method: ClientSecretMethod,
}

impl Default for OAuth2ClientCredentialsAuthState {
    fn default() -> Self {
        Self {
            token_url: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            scope: String::new(),
            client_secret_method: ClientSecretMethod::BasicAuth,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AuthState {
    #[default]
    None,
    Bearer(BearerAuthState),
    ApiKey(ApiKeyAuthState),
    Basic(BasicAuthState),
    OAuth2ClientCredentials(OAuth2ClientCredentialsAuthState),
}

impl AuthState {
    pub const fn kind(&self) -> AuthKind {
        match self {
            Self::None => AuthKind::None,
            Self::Bearer(_) => AuthKind::Bearer,
            Self::ApiKey(_) => AuthKind::ApiKey,
            Self::Basic(_) => AuthKind::Basic,
            Self::OAuth2ClientCredentials(_) => AuthKind::OAuth2ClientCredentials,
        }
    }

    pub fn with_kind(&self, kind: AuthKind) -> Self {
        if self.kind() == kind {
            return self.clone();
        }
        match kind {
            AuthKind::None => Self::None,
            AuthKind::Bearer => Self::Bearer(BearerAuthState::default()),
            AuthKind::ApiKey => Self::ApiKey(ApiKeyAuthState::default()),
            AuthKind::Basic => Self::Basic(BasicAuthState::default()),
            AuthKind::OAuth2ClientCredentials => {
                Self::OAuth2ClientCredentials(OAuth2ClientCredentialsAuthState::default())
            }
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
    match auth {
        AuthState::None | AuthState::OAuth2ClientCredentials(_) => existing.to_string(),
        AuthState::Bearer(bearer) => {
            let mut out = existing.to_string();
            out.push_str("\nAuthorization: Bearer ");
            out.push_str(bearer.token.trim());
            out
        }
        AuthState::ApiKey(api_key) => {
            let mut out = existing.to_string();
            out.push('\n');
            out.push_str(api_key.header_name.trim());
            out.push_str(": ");
            out.push_str(api_key.header_value.trim());
            out
        }
        AuthState::Basic(basic) => {
            let token =
                general_purpose::STANDARD.encode(format!("{}:{}", basic.username, basic.password));
            let mut out = existing.to_string();
            out.push_str("\nAuthorization: Basic ");
            out.push_str(&token);
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthKind, AuthState, BasicAuthState, OAuth2ClientCredentialsAuthState, apply_auth_headers,
    };

    #[test]
    fn auth_state_kind_switches_variant() {
        let auth = AuthState::None.with_kind(AuthKind::OAuth2ClientCredentials);
        assert!(matches!(auth, AuthState::OAuth2ClientCredentials(_)));
    }

    #[test]
    fn oauth2_does_not_apply_static_headers() {
        let auth = AuthState::OAuth2ClientCredentials(OAuth2ClientCredentialsAuthState::default());
        let headers = apply_auth_headers("Accept: application/json", &auth);
        assert_eq!(headers, "Accept: application/json");
    }

    #[test]
    fn basic_auth_header_is_base64_encoded() {
        let auth = AuthState::Basic(BasicAuthState {
            username: "aladdin".to_string(),
            password: "opensesame".to_string(),
        });
        let headers = apply_auth_headers("", &auth);
        assert_eq!(headers, "\nAuthorization: Basic YWxhZGRpbjpvcGVuc2VzYW1l");
    }
}
