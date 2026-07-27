/// Caila (Just AI) LLM provider integration.
///
/// Exposes the OpenAI-compatible adapter at https://caila.io/api/adapters/openai
/// for model discovery (`/models`) and chat completions (handled in
/// `summary::llm_client` via the `LLMProvider::Caila` variant). The key
/// difference from other OpenAI-compatible providers is authentication: Caila
/// expects the raw API key in the `Authorization` header WITHOUT a `Bearer`
/// prefix.
pub mod caila;
