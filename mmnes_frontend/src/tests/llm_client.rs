use mmnes_core::nes_console::NesConsoleError;
use crate::llm_client::{LLMClient, LLMClientError, Prompt};
use crate::openai_llm::OpenAILLM;

const OPENAI_API_URL: &str = "https://api.openai.com/v1/responses";
const OPENAI_MODEL: &str = "gpt-5-nano";

#[ignore]
#[test]
fn openaillm_chat_sends_request_and_receive_response() -> Result<(), LLMClientError> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|e| NesConsoleError::InternalError(format!("OpenAI API key (OPENAI_API_KEY) not set: {}", e))).unwrap();
    let openai = OpenAILLM::new(OPENAI_API_URL, api_key, OPENAI_MODEL)?;
    let prompt = Prompt {
        text: "I'm playing super mario bros, and I'm in world 1-2, are there any warp zones ?".to_string(),
        image: None,
    };
    
    let response = openai.chat(prompt)?;

    println!("response: {}", response);
    Ok(())
}