use std::time::Duration;
use serde_json::{json, Value};
use crate::llm_client::{LLMClient, LLMClientError};

pub struct OpenAILLM {
    client: reqwest::blocking::Client,
    endpoint: String,
    api_key: String,
    model: String,
}

impl LLMClient for OpenAILLM {
    fn chat(&self, prompt: String) -> Result<String, LLMClientError> {
        let request = json!({
            "model": self.model.clone(),
            "instructions": "You are a professional NES gameplay coach. \
                        Your objective is to help the player according to his / her request. \
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
            .map_err(|err| LLMClientError::CommunicationError(format!("could not send request: {}", err)))?;

        let json: Value = resp.json()
            .map_err(|err| LLMClientError::CommunicationError(format!("could not read response {}", err)))?;

        OpenAILLM::parse_response(json)
    }
}

impl OpenAILLM {
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Result<OpenAILLM, LLMClientError> {
        let client = reqwest::blocking::Client::builder()
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

    fn parse_response(json: Value) -> Result<String, LLMClientError> {
        if let Some(text) = json.get("output_text").and_then(|raw| raw.as_str()) {
            return Ok(text.to_string());
        }

        if let Some(array) = json.get("output").and_then(|raw| raw.as_array()) {
            let mut buffer = String::new();

            for item in array {
                if let Some(parts) = item.get("content").and_then(|raw| raw.as_array()) {
                    for part in parts {
                        let content = part.get("type").and_then(|raw| raw.as_str());

                        if content == Some("output_text") || content == Some("text") {
                            if let Some(text) = part.get("text").and_then(|raw| raw.as_str()) {
                                if !text.is_empty() {
                                    buffer.push('\n');
                                }
                                buffer.push_str(text);
                            }
                        }
                    }
                }
            }

            if !buffer.is_empty() {
                return Ok(buffer);
            }
        }

        Err(LLMClientError::CommunicationError("could not find text in response".into()))
    }
}



