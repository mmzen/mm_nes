use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub enum LLMClientError {
    ConfigurationError(String),
    CommunicationError(String),
}

pub trait LLMClient {
    fn chat(&self, prompt: String) -> Result<String, LLMClientError>;
}

impl Display for LLMClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LLMClientError::ConfigurationError(s) => write!(f, "configuration error: {}", s),
            LLMClientError::CommunicationError(s) => write!(f, "communication error: {}", s),
        }
    }
}

