use rex_axum_sdk::firebase_auth::{FirebaseAuthRequest, FirebaseAuthService};
use serde::{Deserialize, Serialize};
// 請求範例
#[derive(Debug, Serialize)]
struct SignInRequest {
    email: String,
    password: String,
    return_secure_token: bool,
}

// 回應範例 - 添加 Serialize trait
#[derive(Debug, Deserialize, Serialize)]
struct SignInResponse {
    id_token: String,
    email: String,
    refresh_token: String,
    expires_in: String,
    local_id: String,
}

impl FirebaseAuthRequest for SignInRequest {
    fn get_endpoint(&self) -> &str {
        "/v1/accounts:signInWithPassword"
    }

    fn req_body(&self) -> serde_json::Value {
        serde_json::json!({
            "email": self.email,
            "password": self.password,
            "returnSecureToken": self.return_secure_token,
        })
    }
}

#[tokio::main]
async fn main() {
    let service = FirebaseAuthService {
        client: reqwest::Client::new(),
        base_url: "https://identitytoolkit.googleapis.com".to_string(),
        api_token: "your-firebase-api-key".to_string(), // 替換成你的 API key
    };

    let sign_in_request = SignInRequest {
        email: "user@example.com".to_string(), // 替換成實際的 email
        password: "password123".to_string(),   // 替換成實際的密碼
        return_secure_token: true,
    };

    match service
        .request::<SignInRequest, SignInResponse>(sign_in_request)
        .await
    {
        Ok(response) => {
            println!("登入成功！");
            println!("Token: {}", response.id_token);
            println!("Email: {}", response.email);
        }
        Err(e) => {
            println!("登入失敗：{:?}", e);
        }
    }
}
