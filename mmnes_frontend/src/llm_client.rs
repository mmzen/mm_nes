
#[derive(Debug, Clone)]
pub enum LLMClientError {
    ConfigurationError(String),
    CommunicationError(String),
}

pub trait LLMClient {
    async fn chat(&self, prompt: String) -> Result<String, LLMClientError>;
}

impl From<reqwest::Error> for LLMClientError {
    fn from(error: reqwest::Error) -> Self {
        LLMClientError::CommunicationError(error.to_string())
    }
}

