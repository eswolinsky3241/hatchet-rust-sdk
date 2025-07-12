use crate::error::HatchetError;
use reqwest::Client;

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
            .get(&url)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(HatchetError::ApiRequestError)?;

        let json = response
            .json::<T>()
            .await
            .map_err(HatchetError::HttpJsonDecode)?;

        Ok(json)
    }
}
