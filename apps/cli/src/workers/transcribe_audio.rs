use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use bratishka_core::{
    events::{EnrichedEvent, EventBus, expect},
    queues::QueueKind,
    workers::{InputSpec, SubscriptionSpec, Worker},
};
use tokio::fs;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    types::{Segment, Transcript},
    workers::events::{AudioTranscribed, YoutubeAudioExtracted},
};

#[derive(Default)]
pub struct TranscribeAudioWorker;

impl TranscribeAudioWorker {
    pub fn new() -> Self {
        Self
    }

    async fn transcribe_audio(
        audio_path: &Path,
        output_path: &Path,
        model_path: &PathBuf,
    ) -> anyhow::Result<Transcript> {
        let mut reader = hound::WavReader::open(audio_path).unwrap();
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / i16::MAX as f32)
            .collect();

        // load a context and model
        let mut ctx_params = WhisperContextParameters {
            use_gpu: true,
            flash_attn: true,
            ..Default::default()
        };
        ctx_params.flash_attn = true;
        let model_path_str = model_path.to_str().unwrap();
        let ctx = WhisperContext::new_with_params(model_path_str, ctx_params)
            .expect("failed to load model");

        // create a params object
        let params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });

        // now we can run the model
        let mut state = ctx.create_state().expect("failed to create state");
        state.full(params, &samples).expect("failed to run model");

        let mut text = String::new();
        let mut segments: Vec<Segment> = Vec::new();

        for segment in state.as_iter() {
            let seg_text = match segment.to_str() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let seg = Segment {
                start: segment.start_timestamp() as f64 / 100.0,
                end: segment.end_timestamp() as f64 / 100.0,
                text: seg_text.to_string(),
            };
            segments.push(seg);

            text.push_str(seg_text);
        }

        let language_index = state.full_lang_id_from_state();
        let language = whisper_rs::get_lang_str(language_index);

        let transcript = Transcript {
            language: language.unwrap_or("Unknown").to_string(),
            segments,
            text,
        };

        fs::write(output_path, serde_json::to_string_pretty(&transcript)?).await?;

        Ok(transcript)
    }

    async fn load_transcript(transcript_path: &std::path::PathBuf) -> anyhow::Result<Transcript> {
        let json_content = fs::read_to_string(transcript_path).await?;
        let transcript: Transcript = serde_json::from_str(&json_content)?;
        Ok(transcript)
    }
}

impl Worker for TranscribeAudioWorker {
    const SUBSCRIBER_ID: &'static str = "transcribe.audio";

    fn subscription() -> SubscriptionSpec {
        SubscriptionSpec {
            subscriber_id: Self::SUBSCRIBER_ID,
            inputs: vec![InputSpec {
                event_type: YoutubeAudioExtracted::EVENT_TYPE,
                queue_kind: QueueKind::FifoDropOldest { capacity: 4 },
            }],
        }
    }

    async fn handle(&mut self, event: Arc<EnrichedEvent>, bus: &EventBus) -> anyhow::Result<()> {
        let req = expect::<YoutubeAudioExtracted>(&event.event, YoutubeAudioExtracted::EVENT_TYPE)?;
        let audio_path = &req.audio_file_path;
        let transcript_path = req.job.cache_dir.join("transcript.json");

        if !req.job.force && transcript_path.exists() {
            let transcript = Self::load_transcript(&transcript_path).await?;
            bus.publish(Arc::new(AudioTranscribed::new(
                event.event.event_id(),
                req.job.clone(),
                transcript,
            )));
            return Ok(());
        }

        let transcript =
            Self::transcribe_audio(&audio_path, &transcript_path, &req.job.model_path).await?;

        bus.publish(Arc::new(AudioTranscribed::new(
            event.event.event_id(),
            req.job.clone(),
            transcript,
        )));

        Ok(())
    }
}
