use std::future::Future;

use rex_axum_sdk::fcm_messaging::{FCMSender, FCMTokenRepository};
use serde_json::json;
use std::error::Error;
// 實作一個簡單的 FCMTokenRepository
struct MyFCMTokenRepository;

impl FCMTokenRepository for MyFCMTokenRepository {
    fn get_user_fcm_token(
        &self,
        user_email: String,
    ) -> impl Future<Output = Result<Option<String>, Box<dyn Error>>> + Send {
        let _ = user_email;
        async move {
            // 在實際應用中，這裡會從資料庫或其他存儲中獲取 token
            Ok(Some("user_fcm_token_123".to_string()))
        }
    }

    fn get_group_fcm_tokens(
        &self,
        group_id: i32,
    ) -> impl Future<Output = Result<Vec<String>, Box<dyn Error>>> + Send {
        let _ = group_id;
        async move {
            // 在實際應用中，這裡會從資料庫獲取群組所有成員的 token
            Ok(vec![
                "group_member1_token".to_string(),
                "group_member2_token".to_string(),
            ])
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 初始化 FCM 發送器
    let mut fcm_sender = FCMSender::new(
        "your-project-id".to_string(),
        "your-access-token".to_string(),
    );

    let repository = MyFCMTokenRepository;

    // 發送給單一用戶
    fcm_sender
        .send_notification_to_user(
            &repository,
            "user@example.com".to_string(),
            "新訊息",
            "您有一則新訊息",
            Some(json!({
                "message_id": "123",
                "sender": "system"
            })),
        )
        .await?;

    // 發送給群組
    fcm_sender
        .send_notifications_to_group(
            &repository,
            1, // group_id
            "群組公告",
            "有新的群組公告",
            Some(json!({
                "announcement_id": "456",
                "type": "group_announcement"
            })),
        )
        .await?;

    // 更新 access token
    fcm_sender.update_access_token("new-access-token".to_string());

    Ok(())
}
