use reqwest::{Client, Error};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
pub trait FirebaseAuthRequest {
    fn get_endpoint(&self) -> &str;
    fn req_body(&self) -> serde_json::Value;
}

#[derive(Debug, Clone)]
pub struct FirebaseAuthService {
    pub client: Client,
    pub base_url: String,
    pub api_token: String,
}

impl FirebaseAuthService {
    pub async fn request<
        T: FirebaseAuthRequest,
        R: DeserializeOwned + std::fmt::Debug + serde::Serialize,
    >(
        &self,
        req: T,
    ) -> Result<R, Error> {
        let url = format!(
            "{}{}?key={}",
            self.base_url,
            &req.get_endpoint(),
            self.api_token
        );
        let response = self
            .client
            .post(url.clone())
            .header("Content-Type", "application/json")
            .json(&req.req_body())
            .send()
            .await?;
        let result = response.json::<R>().await;
        // 使用 serde_json 的 to_string_pretty() 方法格式化输出
        if let Ok(ref data) = result {
            let pretty_json = serde_json::to_string_pretty(data).unwrap();
            tracing::info!("请求回复：\n{}", pretty_json);
        } else {
            tracing::info!("请求回复： {:?}", &result);
        }

        result
    }
}
