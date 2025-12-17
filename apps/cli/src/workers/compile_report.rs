use std::sync::Arc;

use bratishka_core::{
    events::{EnrichedEvent, EventBus, expect},
    queues::QueueKind,
    workers::{InputSpec, SubscriptionSpec, Worker},
};

use crate::{
    provider::{self, Provider},
    types::{Transcript, VideoReport},
    workers::events::{ReportCompiled, SectionsAnalyzed, SourceSection},
};

pub struct CompileReportWorker;

impl CompileReportWorker {
    pub fn new() -> Self {
        Self
    }

    async fn compile_report(
        provider: &Provider,
        transcript: &Transcript,
        sections: &[SourceSection],
        report_lang: &str,
    ) -> anyhow::Result<VideoReport> {
        let config = provider.config();
        let api_key = provider.validate_api_key()?;

        let duration_seconds = transcript.segments.last().map(|s| s.end).unwrap_or(0.0);
        let duration_minutes = duration_seconds / 60.0;

        let system_prompt = format!(
            r#"You are a report compiler with web search access. Synthesize pre-analyzed sections into a comprehensive, easy-to-read report.

  IMPORTANT: Write ALL content in {lang} language.

  INPUT: Pre-analyzed sections with summaries, key_concepts, and external_context

  YOUR TASK:
  1. Find connections and cross-references between sections
  2. Use web search to fill knowledge gaps or add context
  3. Rewrite section summaries to be clearer and more connected
  4. Extract actionable takeaways from all available information
  5. Assess cognitive difficulty (how hard to understand, not technical complexity)

  OUTPUT: Return ONLY valid JSON:
  {{
    "title": "Clear, descriptive video title",
    "summary": "3-4 sentences explaining what viewer will learn and why it matters",
    "duration_minutes": <number>,
    "language": "{lang}",
    "difficulty": "Easy to understand|Moderate cognitive load|Cognitively demanding",
    "key_takeaways": [
      "Actionable insight 1 (what viewer should do/remember)",
      "Actionable insight 2",
      "..."
    ],
    "sections": [
      {{
        "start_seconds": 0.0,
        "end_seconds": 180.0,
        "title": "Section title",
        "summary": "Refined summary with cross-references and enriched context. Example: 'This builds on the concept from Section 2...'"
      }}
    ]
  }}

  RULES:
  - Cross-reference related concepts across sections in summaries
  - Use web search when you need to clarify complex terms or add context
  - Key takeaways = 5-7 actionable insights (what to DO, not just what was said)
  - Difficulty based on: concept density, abstraction level, prerequisite knowledge needed
  - Rewrite section summaries to be self-contained but connected
  - Focus on making content easy to understand and retain
  - Output ONLY JSON, nothing else"#,
            lang = report_lang
        );

        let prepared_sections = serde_json::to_string_pretty(&sections)?;

        let user_prompt = format!(
            "Analyze this video transcript (duration: {:.1} minutes, language: {}):\n\n{}",
            duration_minutes, transcript.language, prepared_sections
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
                        "content": &system_prompt,
                    },
                    {
                        "role": "user",
                        "content": user_prompt,
                    },
                ],
                "temperature": 0.3,
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        // Extract content from response - /v1/responses format
        let content = response["output"]
            .as_array()
            .and_then(|arr| arr.iter().rev().find(|item| item["type"] == "message"))
            .and_then(|msg| msg["content"][0]["text"].as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(format!("Invalid API response structure: {:?}", response))
            })?;

        // Parse JSON content into VideoReport
        let report: VideoReport = serde_json::from_str(content)?;

        Ok(report)
    }
}

impl Worker for CompileReportWorker {
    const SUBSCRIBER_ID: &'static str = "compile.report";

    fn subscription() -> SubscriptionSpec {
        SubscriptionSpec {
            subscriber_id: Self::SUBSCRIBER_ID,
            inputs: vec![InputSpec {
                event_type: SectionsAnalyzed::EVENT_TYPE,
                queue_kind: QueueKind::FifoDropOldest { capacity: 4 },
            }],
        }
    }

    async fn handle(&mut self, event: Arc<EnrichedEvent>, bus: &EventBus) -> anyhow::Result<()> {
        let req = expect::<SectionsAnalyzed>(&event.event, SectionsAnalyzed::EVENT_TYPE)?;
        let lang = if let Some(lang) = &req.job.requested_report_lang {
            lang
        } else {
            &req.transcript.language.clone()
        };

        let report =
            Self::compile_report(&req.job.provider, &req.transcript, &req.sections, &lang).await?;

        bus.publish(Arc::new(ReportCompiled::new(
            event.event.event_id(),
            req.job.clone(),
            report,
        )));
        todo!()
    }
}
