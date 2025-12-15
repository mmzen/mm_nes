// Authorship: Human 100% | Claude 0%
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub enum LLMClientError {
    ConfigurationError(String),
    CommunicationError(String),
}

pub trait LLMClient {
    fn chat(&self, prompt: Prompt) -> Result<String, LLMClientError>;
}

pub struct Prompt {
    pub text: String,
    pub image: Option<Vec<u8>>,
    pub rom_title: Option<String>,
}

impl Prompt {
    pub fn new(text: String, image: Option<Vec<u8>>, rom_title: Option<String>) -> Self {
        Prompt {
            text,
            image,
            rom_title
        }
    }
}

impl Display for LLMClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LLMClientError::ConfigurationError(s) => write!(f, "configuration error: {}", s),
            LLMClientError::CommunicationError(s) => write!(f, "communication error: {}", s),
        }
    }
}

