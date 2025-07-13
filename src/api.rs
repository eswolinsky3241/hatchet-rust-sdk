use reqwest::Client;

use crate::error::HatchetError;

pub(crate) struct ApiClient {
    base_url: String,
    token: String,
    http_client: Client,
}

impl ApiClient {
    pub(crate) fn new(base_url: String, token: String) -> Self {
        let http_client = Client::builder()
            .build()
            .expect("Failed to build reqwest client");

        Self {
            base_url,
            token,
            http_client,
        }
    }

    pub async fn get<T>(&self, path: &str) -> Result<T, HatchetError>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );

        let response = self
            .http_client
            .get("https://cloud.onhatchet.run/api/v1/stable/workflow-runs/42ebd4a1-cf65-4cd5-9c5c-4e86809ff2b2")
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(HatchetError::ApiRequestError)?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(HatchetError::ApiRequestError)?;

        if !status.is_success() {
            return Err(HatchetError::HttpError {
                url: url.to_string(),
                method: reqwest::Method::GET,
                status,
                body: body.clone(),
            });
        }

        let json = serde_json::from_str::<T>(&body).map_err(|_e| HatchetError::JsonParseError {
            status,
            body: body.clone(),
        })?;

        Ok(json)
    }
}
