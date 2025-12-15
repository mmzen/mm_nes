// Authorship: Human 100% | Claude 0%
use std::fmt::{Display, Formatter};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use log::info;
use crate::llm_client::{LLMClient, LLMClientError, Prompt};
use crate::openai_llm::OpenAILLM;

pub enum AiWorkerError {
    InternalError(String),
    CommunicationError(String),
}

impl Display for AiWorkerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AiWorkerError::InternalError(s) => { write!(f, "internal error: {}", s) },
            AiWorkerError::CommunicationError(s) => { write!(f, "communication error: {}", s) },
        }
    }
}

pub enum AiWorkMessage {
    Reply { text: String },
    Error { text: String },
}

struct AiRequest {
    prompt: Prompt,
}

pub struct AiWorker {
    request_tx: Sender<AiRequest>,
    message_rx: Receiver<AiWorkMessage>,
    #[allow(dead_code)]
    handle: Option<JoinHandle<()>>,
}

impl AiWorker {

    fn loop_not_ready(request_rx: Receiver<AiRequest>, message_tx: Sender<AiWorkMessage>, error: LLMClientError) {
        info!("AI worker running but not ready...: {}", error);

        loop {
            let response = match request_rx.recv() {
                _ => AiWorkMessage::Reply { text: "AI is not initialized and can not answer".to_string() },
            };

            let _ = message_tx.send(response);
        }
    }

    fn loop_ready(request_rx: Receiver<AiRequest>, message_tx: Sender<AiWorkMessage>, client: OpenAILLM) {
        info!("AI worker running ...");

        loop {
            if let Ok(AiRequest { prompt }) = request_rx.recv() {
                let response = match client.chat(prompt) {
                    Ok(text) => AiWorkMessage::Reply { text },
                    Err(e) => AiWorkMessage::Error { text: format!("OpenAI request failed: {}", e) },
                };

                let _ = message_tx.send(response);
            }
        }
    }

    pub fn spawn(api_key: String, api_url: &str, model: &str) -> Result<AiWorker, AiWorkerError> {
        let (request_tx, request_rx) = channel::<AiRequest>();
        let (message_tx, message_rx) = channel::<AiWorkMessage>();

        /***
         *  should be injected and be a dynamic trait
         ***/
        let client = OpenAILLM::new(api_url, api_key, model);

        let handle = thread::Builder::new()
            .name("ai_worker".to_string())
            .spawn(move || {
                match client {
                    Ok(client) => AiWorker::loop_ready(request_rx, message_tx, client),
                    Err(error) => AiWorker::loop_not_ready(request_rx, message_tx, error),
                }
            })
            .map_err(|e| AiWorkerError::InternalError(e.to_string()))?;

        info!("AI worker started...");
        Ok(AiWorker { request_tx, message_rx, handle: Some(handle) })
    }

    pub fn request(&self, prompt: Prompt) -> Result<(), AiWorkerError> {
        self.request_tx.send(AiRequest { prompt })
            .map_err(|e| AiWorkerError::CommunicationError(e.to_string()))
    }

    pub fn try_recv(&self) -> Option<AiWorkMessage> {
        self.message_rx.try_recv().ok()
    }
}

