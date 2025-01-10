use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{error::Error, future::Future};

mod models {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub(crate) struct FCMMessage {
        pub message: Message,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub(crate) struct Message {
        pub token: String,
        pub notification: Notification,
        pub data: Option<Value>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub(crate) struct Notification {
        pub title: String,
        pub body: String,
    }
}

// 定義一個錯誤類型用於不支援的操作
#[derive(Debug)]
pub struct UnsupportedOperationError;

impl std::fmt::Display for UnsupportedOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "This operation is not supported")
    }
}

impl Error for UnsupportedOperationError {}

pub trait FCMTokenRepository {
    fn get_user_fcm_token(
        &self,
        user_email: String,
    ) -> impl Future<Output = Result<Option<String>, Box<dyn Error>>> + Send;

    // 預設實現，回傳不支援的錯誤
    fn get_group_fcm_tokens(
        &self,
        _group_id: i32,
    ) -> impl Future<Output = Result<Vec<String>, Box<dyn Error>>> + Send {
        async { Err(Box::new(UnsupportedOperationError) as Box<dyn Error>) }
    }
}

#[derive(Clone, Debug)]
pub struct FCMSender {
    client: Client,
    project_id: String,
    access_token: String,
}

impl FCMSender {
    pub fn new(project_id: String, access_token: String) -> Self {
        Self {
            client: Client::new(),
            project_id,
            access_token,
        }
    }

    pub fn update_access_token(&mut self, token: String) {
        self.access_token = token;
    }

    async fn send_fcm_message(
        &self,
        token: &str,
        title: &str,
        body: &str,
        data: Option<Value>,
    ) -> Result<(), Box<dyn Error>> {
        use models::*;

        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            self.project_id
        );

        let message = FCMMessage {
            message: Message {
                token: token.to_string(),
                notification: Notification {
                    title: title.to_string(),
                    body: body.to_string(),
                },
                data,
            },
        };

        self.client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&message)
            .send()
            .await?;

        Ok(())
    }

    pub async fn send_notification_to_user(
        &self,
        repository: &impl FCMTokenRepository,
        user_email: String,
        title: &str,
        body: &str,
        data: Option<Value>,
    ) -> Result<(), Box<dyn Error>> {
        let token = repository
            .get_user_fcm_token(user_email)
            .await?
            .ok_or("User does not have an FCM token")?;

        self.send_fcm_message(&token, title, body, data).await
    }

    pub async fn send_notifications_to_group(
        &self,
        repository: &impl FCMTokenRepository,
        group_id: i32,
        title: &str,
        body: &str,
        data: Option<Value>,
    ) -> Result<(), Box<dyn Error>> {
        let tokens = repository.get_group_fcm_tokens(group_id).await?;

        for token in tokens {
            if let Err(e) = self
                .send_fcm_message(&token, title, body, data.clone())
                .await
            {
                eprintln!("Failed to send notification: {}", e);
            }
        }

        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // 基本的測試用 Repository
    struct TestTokenRepository {
        user_token: Option<String>,
    }

    impl TestTokenRepository {
        fn new(user_token: Option<String>) -> Self {
            Self { user_token }
        }
    }

    impl FCMTokenRepository for TestTokenRepository {
        fn get_user_fcm_token(
            &self,
            _user_email: String,
        ) -> impl Future<Output = Result<Option<String>, Box<dyn Error>>> + Send {
            let token = self.user_token.clone();
            async move { Ok(token) }
        }
        // 使用預設的 get_group_fcm_tokens 實現
    }

    // 完整功能的測試用 Repository
    struct TestFullRepository {
        user_token: Option<String>,
        group_tokens: Vec<String>,
    }

    impl TestFullRepository {
        fn new(user_token: Option<String>, group_tokens: Vec<String>) -> Self {
            Self {
                user_token,
                group_tokens,
            }
        }
    }

    impl FCMTokenRepository for TestFullRepository {
        fn get_user_fcm_token(
            &self,
            _user_email: String,
        ) -> impl Future<Output = Result<Option<String>, Box<dyn Error>>> + Send {
            let token = self.user_token.clone();
            async move { Ok(token) }
        }

        fn get_group_fcm_tokens(
            &self,
            _group_id: i32,
        ) -> impl Future<Output = Result<Vec<String>, Box<dyn Error>>> + Send {
            let tokens = self.group_tokens.clone();
            async move { Ok(tokens) }
        }
    }

    #[tokio::test]
    async fn test_send_notification_to_user_no_token() {
        let repo = TestTokenRepository::new(None);

        let sender = FCMSender::new("test-project".to_string(), "test-token".to_string());

        let result = sender
            .send_notification_to_user(
                &repo,
                "test@example.com".to_string(),
                "Test Title",
                "Test Body",
                None,
            )
            .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "User does not have an FCM token"
        );
    }

    #[tokio::test]
    async fn test_send_notifications_to_group() {
        let repo = TestFullRepository::new(None, vec!["token1".to_string(), "token2".to_string()]);

        let sender = FCMSender::new("test-project".to_string(), "test-token".to_string());

        let result = sender
            .send_notifications_to_group(&repo, 1, "Test Title", "Test Body", None)
            .await;

        // 因為我們在 send_notifications_to_group 中忽略了單個發送的錯誤
        // 所以即使無法連接到 FCM 服務，整體結果仍然是 Ok
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_group_notification_not_supported() {
        let repo = TestTokenRepository::new(Some("test_token".to_string()));

        let sender = FCMSender::new("test-project".to_string(), "test-token".to_string());

        let result = sender
            .send_notifications_to_group(&repo, 1, "Test Title", "Test Body", None)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not supported"), "Unexpected error: {}", err);
    }
}
