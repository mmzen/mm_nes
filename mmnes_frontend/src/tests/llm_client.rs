use crate::llm_client::{LLMClient, LLMClientError};
use crate::openai_llm::OpenAILLM;

const OPENAI_API_KEY: &str = "[redacted]";
const OPENAI_API_URL: &str = "https://api.openai.com/v1/responses";
const OPENAI_MODEL: &str = "gpt-5-nano";

#[ignore]
#[tokio::test]
async fn openaillm_chat_sends_request_and_receive_response() -> Result<(), LLMClientError> {
    let openai = OpenAILLM::new(OPENAI_API_URL, OPENAI_API_KEY, OPENAI_MODEL)?;
    let response = openai.chat("I'm playing super mario bros, and I'm in world 1-2, are there any warp zones ?".to_string());

    println!("response: {}", response.await?);
    Ok(())
}