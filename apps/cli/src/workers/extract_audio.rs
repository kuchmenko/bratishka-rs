use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use bratishka_core::{
    events::{EnrichedEvent, Event, EventBus, expect},
    queues::QueueKind,
    workers::{InputSpec, PipelineFailed, SubscriptionSpec, Worker},
};
use tokio::process::Command;

use crate::workers::events::{YoutubeAudioExtracted, YoutubeVideoDownloaded};

pub struct ExtractAudioWorker;

impl ExtractAudioWorker {
    pub fn new() -> Self {
        Self
    }

    async fn extract_audio(video_path: &Path, audio_path: &Path) -> anyhow::Result<()> {
        let output = Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(video_path)
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg(audio_path)
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to extract audio from video file"));
        }

        Ok(())
    }

    fn get_audio_path(cache_dir: &Path) -> PathBuf {
        cache_dir.join("audio.wav")
    }
}

impl Worker for ExtractAudioWorker {
    const SUBSCRIBER_ID: &'static str = "youtube.extract_audio";

    fn subscription() -> SubscriptionSpec {
        SubscriptionSpec {
            subscriber_id: Self::SUBSCRIBER_ID,
            inputs: vec![InputSpec {
                event_type: YoutubeVideoDownloaded::EVENT_TYPE,
                queue_kind: QueueKind::FifoDropOldest { capacity: 4 },
            }],
        }
    }

    async fn handle(&mut self, event: Arc<EnrichedEvent>, bus: &EventBus) -> anyhow::Result<()> {
        let req =
            expect::<YoutubeVideoDownloaded>(&event.event, YoutubeVideoDownloaded::EVENT_TYPE)?;

        let audio_path = Self::get_audio_path(&req.job.cache_dir);
        let cached = !req.job.force && audio_path.exists();

        if !cached && let Err(e) = Self::extract_audio(&req.video_file_path, &audio_path).await {
            bus.publish(Arc::new(PipelineFailed::new(
                Arc::clone(&event.event),
                Self::SUBSCRIBER_ID,
                format!("{}", e),
            )));

            return Ok(());
        }

        bus.publish(Arc::new(YoutubeAudioExtracted::new(
            req.event_id(),
            req.job.clone(),
            audio_path,
        )));
        Ok(())
    }
}
