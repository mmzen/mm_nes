use std::fmt::{Display, Formatter};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use log::info;
use crate::llm_client::{LLMClient, LLMClientError};
use crate::openai_llm::OpenAILLM;

pub enum AiWorkerError {
    ClientInitializationError(LLMClientError),
    InternalError(String),
    CommunicationError(String),
}

impl Display for AiWorkerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AiWorkerError::ClientInitializationError(e) => { write!(f, "client initialization error: {}", e) },
            AiWorkerError::InternalError(s) => { write!(f, "internal error: {}", s) },
            AiWorkerError::CommunicationError(s) => { write!(f, "communication error: {}", s) },
        }
    }
}

pub enum AiWorkMessage {
    Reply { id: u32, text: String },
    Error { id: u32, text: String },
}

struct AiRequest {
    id: u32,
    prompt: String,
}

pub struct AiWorker {
    request_tx: Sender<AiRequest>,
    message_rx: Receiver<AiWorkMessage>,
    handle: Option<JoinHandle<()>>,
}

impl AiWorker {

    pub fn spawn(api_key: String, api_url: &str, model: &str) -> Result<AiWorker, AiWorkerError> {
        let (request_tx, request_rx) = channel::<AiRequest>();
        let (message_tx, message_rx) = channel::<AiWorkMessage>();

        let client = OpenAILLM::new(api_url, api_key, model)
            .map_err(|e| AiWorkerError::ClientInitializationError(e))?;

        let handle = thread::Builder::new()
            .name("ai_worker".to_string())
            .spawn(move || {
                while let Ok(AiRequest { id, prompt }) = request_rx.recv() {
                    let response = match client.chat(prompt) {
                        Ok(text) => AiWorkMessage::Reply { id, text },
                        Err(e) => AiWorkMessage::Error { id, text: format!("OpenAI request failed: {}", e) },
                    };

                    let _ = message_tx.send(response);
                }
            })
            .map_err(|e| AiWorkerError::InternalError(e.to_string()))?;

        info!("AI worker started...");

        Ok(AiWorker { request_tx, message_rx, handle: Some(handle) })
    }

    pub fn request(&self, id: u32, prompt: String) -> Result<(), AiWorkerError> {
        self.request_tx.send(AiRequest { id, prompt })
            .map_err(|e| AiWorkerError::CommunicationError(e.to_string()))
    }

    pub fn try_recv(&self) -> Option<AiWorkMessage> {
        self.message_rx.try_recv().ok()
    }
}

