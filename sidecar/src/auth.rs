use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("origin not allowed")]
    OriginNotAllowed,
    #[error("authorization header missing or malformed")]
    Unauthorized,
}

impl AuthError {
    pub fn status_code(self) -> StatusCode {
        match self {
            AuthError::OriginNotAllowed => StatusCode::FORBIDDEN,
            AuthError::Unauthorized => StatusCode::UNAUTHORIZED,
        }
    }
}

pub fn check_origin(headers: &HeaderMap) -> Result<(), AuthError> {
    if let Some(origin) = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    {
        if origin != "null" {
            return Err(AuthError::OriginNotAllowed);
        }
    }
    Ok(())
}

pub fn check_bearer_token(headers: &HeaderMap, expected_token: &str) -> Result<(), AuthError> {
    let auth_header = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok());
    let Some(bearer) = auth_header else {
        return Err(AuthError::Unauthorized);
    };

    let Some(provided) = bearer.strip_prefix("Bearer ") else {
        return Err(AuthError::Unauthorized);
    };

    if subtle_equals(provided.as_bytes(), expected_token.as_bytes()) {
        Ok(())
    } else {
        Err(AuthError::Unauthorized)
    }
}

/// Constant-time comparison to avoid timing leaks.
fn subtle_equals(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    a.ct_eq(b).into()
}
