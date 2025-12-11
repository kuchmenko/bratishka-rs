#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Missing API key for {provider_name}")]
    MissingApiKey { provider_name: String },
}

#[derive(Clone, Debug, Default)]
pub enum Provider {
    #[default]
    Grok,
    Openai,
    Gemini,
}

pub struct ProviderConfig {
    pub api_url: &'static str,
    pub model: &'static str,
    pub env_var: &'static str,
}

impl Provider {
    pub fn config(&self) -> ProviderConfig {
        match self {
            Provider::Grok => ProviderConfig {
                api_url: "https://api.x.ai/v1/chat/completions",
                model: "grok-4-fast",
                env_var: "XAI_API_KEY",
            },
            Provider::Openai => ProviderConfig {
                api_url: "https://api.openai.com/v1/chat/completions",
                model: "gpt-5.1",
                env_var: "OPENAI_API_KEY",
            },
            Provider::Gemini => ProviderConfig {
                api_url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                model: "gemini-3-pro",
                env_var: "GEMINI_API_KEY",
            },
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Provider::Grok => "Grok",
            Provider::Openai => "OpenAI",
            Provider::Gemini => "Gemini",
        }
    }

    /// Validate that the API key is set for this provider
    pub fn validate_api_key(&self) -> Result<String, ProviderError> {
        let config = self.config();
        std::env::var(config.env_var).map_err(|_| ProviderError::MissingApiKey {
            provider_name: self.name().to_string(),
        })
    }
}
