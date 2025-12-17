use std::sync::Arc;

use bratishka_core::{
    events::expect,
    queues::{FifoDropOldestQueue, QueueKind},
    workers::{InputSpec, SubscriptionSpec, Worker},
};
use serde::{Deserialize, Serialize};

use crate::{
    provider::{Provider, ProviderError},
    types::Transcript,
    workers::events::{AudioTranscribed, SectionsAnalyzed, SourceSection},
};

static SECTIONS_ANALYSIS_PROMPT: &str = r#"
  You are a video content analyzer. You have access to web search to enrich your analysis.

  INPUT: Video transcript with timestamps in format [MM:SS] text

  TASK:
  1. Identify logical sections based on topic changes
  2. For each section, use web search to find context on technical terms, concepts, or
  references mentioned
  3. Create detailed summaries that combine transcript content with external knowledge

  OUTPUT: Return ONLY valid JSON array:
  [
    {
      "name": "Section title",
      "content": "Raw transcript text for this section",
      "started_at": 0.0,
      "ended_at": 125.5,
      "key_concepts": ["concept1", "concept2"],
      "external_context": "Relevant background from web search",
      "summary": "1-2 paragraph detailed summary. Explain technical terms. Include context
  not in transcript. Connect concepts to broader knowledge."
    }
  ]

  RULES:
  - Identify 3-10 sections based on topic changes
  - Use web search when you encounter:
    - Technical terms or jargon
    - Named technologies, frameworks, protocols
    - References to events, people, companies
    - Concepts that need explanation
  - Sections must be sequential and cover entire video
  - Summary should educate, not just describe
"#;

pub struct AnalyzeSectionsWorker;

#[derive(Debug, thiserror::Error)]
pub enum InteligenceError {
    #[error("Provider error: {0}")]
    ProviderError(#[from] ProviderError),

    #[error("Invalid API response: {0}")]
    InvalidApiResponse(serde_json::Value),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Failed to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Failed to process sections: {reason}")]
    ProcessSectionsFailed { reason: String },
}

impl AnalyzeSectionsWorker {
    pub fn new() -> Self {
        Self
    }

    async fn analyze_sections(
        provider: &Provider,
        transcript: &Transcript,
    ) -> anyhow::Result<Vec<SourceSection>> {
        let config = provider.config();
        let api_key = provider.validate_api_key()?;
        let user_prompt = format!(
            "Attaching the transcript and timestamps. {}",
            serde_json::to_string_pretty(transcript)?
        );

        let response = reqwest::Client::new()
            .post(config.api_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": config.model,
                "tools": [{"type": "web_search"}],
                "input": [
                    {
                        "role": "system",
                        "content": SECTIONS_ANALYSIS_PROMPT,
                    },
                    {
                        "role": "user",
                        "content": user_prompt,
                    },
                ],
                "temperature": 0.3,
            }))
            .send()
            .await?;
        println!("RESPONSE: {:?}", response);

        let response = response.json::<serde_json::Value>().await?;

        // Extract content from response - /v1/responses format
        let content = response["output"]
            .as_array()
            .and_then(|arr| arr.iter().rev().find(|item| item["type"] == "message"))
            .and_then(|msg| msg["content"][0]["text"].as_str())
            .ok_or_else(|| InteligenceError::ProcessSectionsFailed {
                reason: format!("Invalid API response structure: {:?}", response),
            })?;

        Ok(serde_json::from_str(content)?)
    }
}

impl Worker for AnalyzeSectionsWorker {
    const SUBSCRIBER_ID: &'static str = "analyze.sections";

    fn subscription() -> bratishka_core::workers::SubscriptionSpec {
        SubscriptionSpec {
            subscriber_id: Self::SUBSCRIBER_ID,
            inputs: vec![InputSpec {
                event_type: AudioTranscribed::EVENT_TYPE,
                queue_kind: QueueKind::FifoDropOldest { capacity: 4 },
            }],
        }
    }

    async fn handle(
        &mut self,
        event: std::sync::Arc<bratishka_core::events::EnrichedEvent>,
        bus: &bratishka_core::events::EventBus,
    ) -> anyhow::Result<()> {
        let req = expect::<AudioTranscribed>(&event.event, AudioTranscribed::EVENT_TYPE)?;
        let sections = Self::analyze_sections(&req.job.provider, &req.transcript).await?;

        bus.publish(Arc::new(SectionsAnalyzed::new(
            event.event.event_id(),
            req.job.clone(),
            sections,
            req.transcript.clone(),
        )));

        Ok(())
    }
}
