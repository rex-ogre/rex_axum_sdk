use axum::{
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use rex_axum_sdk::utilty::{extract_jwt_token, Claims, JwtAuth}; // 注意這裡加入 extract_jwt_token
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;

// 受保護的資源需要 JWT 認證
async fn protected_route(claims: JwtAuth) -> impl IntoResponse {
    Json(json!({
        "message": "這是受保護的資源",
        "user_email": claims.0.email,
        "user_id": claims.0.sub
    }))
}

// 公開路由，不需要認證
async fn public_route() -> impl IntoResponse {
    Json(json!({
        "message": "這是公開的資源"
    }))
}

// 登入請求的資料結構
#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

// 登入回應的資料結構
#[derive(Debug, Serialize)]
struct LoginResponse {
    token: String,
}

// 模擬登入處理 - 現在會產生真實的 JWT token
async fn login_handler(Json(payload): Json<LoginRequest>) -> Response {
    // 在實際應用中，這裡應該要驗證用戶憑證
    let claims = Claims::mock(); // 使用 mock claims 作為示範

    // 生成 JWT token
    // 注意：在實際應用中，這個密鑰應該從配置中讀取
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret("your-secret".as_bytes()),
    )
    .expect("Failed to create token");

    Json(LoginResponse { token }).into_response()
}

// 建立 Router
pub fn create_router() -> Router {
    // 創建一個需要認證的路由群組
    let protected = Router::new()
        .route("/protected", get(protected_route))
        .layer(middleware::from_extractor::<JwtAuth>());

    // 創建公開的路由群組
    let public = Router::new()
        .route("/public", get(public_route))
        .route("/login", post(login_handler));

    // 合併路由
    Router::new().merge(protected).merge(public)
}

#[tokio::main]
async fn main() {
    // 初始化 tracing
    tracing_subscriber::fmt::init();

    // 建立 app
    let app = create_router();

    // 運行伺服器
    let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
