use std::time::Duration;
use serde_json::json;
use crate::llm_client::{LLMClient, LLMClientError};

pub struct OpenAILLM {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: String,
}

impl LLMClient for OpenAILLM {
    async fn chat(&self, prompt: String) -> Result<String, LLMClientError> {
        let request = json!({
            "model": self.model.clone(),
            "instructions": "You are a professional NES gameplay coach. \
                        Your objective is to help the player by giving hints and revealing secrets. \
                        Give one short hint on the specified game. \
                        Propose no follow-up, answer must be relatively concise as it will be read by the player during gameplay.".to_string(),
            "input": prompt,
            "reasoning": {
                "effort": "low"
            },
            "store": false,
        });

        let resp = self.client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await?;

        Ok(resp.text().await?)
    }
}

impl OpenAILLM {
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Result<OpenAILLM, LLMClientError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .or(Err(LLMClientError::ConfigurationError("could not build HTTP client".to_string())))?;

        let openai = OpenAILLM {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            client
        };

        Ok(openai)
    }
}



