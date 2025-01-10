use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use chrono::{Duration, Utc};
use hyper::StatusCode;
use jsonwebtoken::{decode, Algorithm, DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    aud: String,
    exp: usize,
    iat: usize,
    pub email: String,
    pub name: Option<String>, // 其他你想要提取的字段
}

impl Claims {
    pub fn mock() -> Self {
        let now = Utc::now();
        let iat = now.timestamp() as usize;
        let exp = (now + Duration::hours(1)).timestamp() as usize;

        Claims {
            sub: "1234567890".to_string(),
            aud: "example_audience".to_string(),
            exp,
            iat,
            email: "user@example.com".to_string(),
            name: Some("John Doe".to_string()),
        }
    }
}

#[derive(Debug)]
pub enum JwtError {
    NoValidKeyError,
    ValidationError(jsonwebtoken::errors::Error),
    MissingToken,
    InvalidToken,
    FetchError(reqwest::Error),
}

impl fmt::Display for JwtError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JwtError::NoValidKeyError => write!(f, "No valid public key found"),
            JwtError::ValidationError(e) => write!(f, "JWT validation error: {}", e),
            JwtError::MissingToken => write!(f, "Missing authorization token"),
            JwtError::InvalidToken => write!(f, "Invalid token"),
            JwtError::FetchError(e) => write!(f, "Failed to fetch public keys: {}", e),
        }
    }
}

impl Error for JwtError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            JwtError::ValidationError(e) => Some(e),
            JwtError::FetchError(e) => Some(e),
            _ => None,
        }
    }
}

impl axum::response::IntoResponse for JwtError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            JwtError::NoValidKeyError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "No valid public key found",
            ),
            JwtError::ValidationError(_) => (StatusCode::UNAUTHORIZED, "Invalid token"),
            JwtError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing token"),
            JwtError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            JwtError::FetchError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch public keys",
            ),
        };

        (status, error_message).into_response()
    }
}

pub async fn extract_jwt_token(token: String) -> Result<TokenData<Claims>, JwtError> {
    let public_keys = fetch_firebase_public_keys()
        .await
        .map_err(JwtError::FetchError)?;

    for (_, key) in public_keys {
        let decoding_key =
            DecodingKey::from_rsa_pem(key.as_bytes()).map_err(JwtError::ValidationError)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&["leaveanote-4af85"]);
        match decode::<Claims>(&token, &decoding_key, &validation) {
            Ok(token_data) => {
                tracing::info!("Token 驗證成功，數據: {:?}", token_data.claims);
                return Ok(token_data);
            }
            Err(e) => {
                tracing::debug!("嘗試解碼失敗，嘗試下一個公鑰: {:?}", e);
                continue;
            }
        }
    }
    tracing::warn!("所有公鑰均無法驗證 Token");
    Err(JwtError::NoValidKeyError)
}

pub async fn fetch_firebase_public_keys(
) -> Result<std::collections::HashMap<String, String>, reqwest::Error> {
    let url =
        "https://www.googleapis.com/robot/v1/metadata/x509/securetoken@system.gserviceaccount.com";
    let response = reqwest::get(url).await?.json().await?;
    Ok(response)
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtAuth(pub Claims);

impl JwtAuth {
    pub fn new() -> Self {
        JwtAuth(Claims {
            sub: "".to_string(),
            aud: "".to_string(),
            exp: 0,
            iat: 0,
            email: "".to_string(),
            name: None,
        })
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for JwtAuth
where
    S: Send + Sync,
{
    type Rejection = JwtError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .ok_or(JwtError::MissingToken)?
            .to_str()
            .map_err(|_| JwtError::InvalidToken)?;

        if !auth_header.starts_with("Bearer ") {
            return Err(JwtError::InvalidToken);
        }

        let token = &auth_header["Bearer ".len()..];
        let token_data = extract_jwt_token(token.to_string()).await?;
        Ok(JwtAuth(token_data.claims))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse; // 改為
    #[tokio::test]
    async fn test_claims_mock() {
        let claims = Claims::mock();

        // 驗證基本欄位
        assert_eq!(claims.sub, "1234567890");
        assert_eq!(claims.email, "user@example.com");
        assert_eq!(claims.name, Some("John Doe".to_string()));

        // 驗證時間戳
        assert!(claims.exp > claims.iat);

        // 驗證過期時間是否在一小時後
        assert_eq!(claims.exp - claims.iat, 3600);
    }

    #[tokio::test]
    async fn test_jwt_auth_new() {
        let jwt_auth = JwtAuth::new();

        // 驗證默認值
        assert_eq!(jwt_auth.0.sub, "");
        assert_eq!(jwt_auth.0.email, "");
        assert_eq!(jwt_auth.0.name, None);
        assert_eq!(jwt_auth.0.exp, 0);
        assert_eq!(jwt_auth.0.iat, 0);
        assert_eq!(jwt_auth.0.aud, "");
    }

    #[tokio::test]
    async fn test_jwt_error_display() {
        // 測試各種錯誤類型的顯示文本
        let errors = vec![
            (JwtError::MissingToken, "Missing authorization token"),
            (JwtError::InvalidToken, "Invalid token"),
            (JwtError::NoValidKeyError, "No valid public key found"),
        ];

        for (error, expected_message) in errors {
            assert_eq!(error.to_string(), expected_message);
        }
    }

    #[tokio::test]
    async fn test_jwt_error_response_status() {
        // 測試錯誤轉換為 HTTP 響應的狀態碼
        let test_cases = vec![
            (JwtError::MissingToken, StatusCode::UNAUTHORIZED),
            (JwtError::InvalidToken, StatusCode::UNAUTHORIZED),
            (JwtError::NoValidKeyError, StatusCode::INTERNAL_SERVER_ERROR),
        ];

        for (error, expected_status) in test_cases {
            let response = error.into_response();
            assert_eq!(response.status(), expected_status);
        }
    }

    #[tokio::test]
    async fn test_firebase_public_keys_fetch() {
        // 注意：這是一個實際的網絡請求，在實際的 CI/CD 環境中可能需要 mock
        let result = fetch_firebase_public_keys().await;
        assert!(result.is_ok());

        if let Ok(keys) = result {
            assert!(!keys.is_empty());
            // 驗證所有的key都是有效的 PEM 格式
            for (_, key) in keys {
                assert!(key.contains("BEGIN"));
                assert!(key.contains("END"));
            }
        }
    }

    #[test]
    fn test_jwt_error_source() {
        // 測試 Error trait 的實現
        let error = JwtError::NoValidKeyError;
        assert!(error.source().is_none());

        // 測試包裝其他錯誤的情況
        let jwt_error =
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken);
        let error = JwtError::ValidationError(jwt_error);
        assert!(error.source().is_some());
    }
}
