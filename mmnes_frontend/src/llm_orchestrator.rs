use crate::llm_client::LLMClient;

pub struct LLMOrchestrator<C: LLMClient> {
    client: C,
    max_token_per_min: u16,
    max_request_per_min: u16,
}