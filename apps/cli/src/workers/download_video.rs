use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use bratishka_core::{
    events::{EnrichedEvent, EventBus, expect},
    queues::QueueKind,
    workers::{InputSpec, SubscriptionSpec, Worker},
};
use tokio::process::Command;

use crate::workers::events::{YoutubeUrlRequested, YoutubeVideoDownloaded};

pub struct DownloadVideoWorker;

impl DownloadVideoWorker {
    pub fn new() -> Self {
        Self
    }

    pub async fn download_video(url: &str, cache_dir: &Path) -> anyhow::Result<PathBuf> {
        let output_template = cache_dir.join("video.%(ext)s");
        let output = Command::new("yt-dlp")
            .arg(&url)
            .arg("--print")
            .arg("after_move:filepath")
            .arg("--extractor-args")
            .arg("youtube:player_client=android,web")
            .arg("-f")
            .arg("best")
            .arg("-o")
            .arg(&output_template)
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("{}", output.status));
        }

        let stdout_str = String::from_utf8_lossy(output.stdout.as_slice());
        let filepath = stdout_str.trim();

        Ok(filepath.into())
    }
}

impl Worker for DownloadVideoWorker {
    const SUBSCRIBER_ID: &'static str = "youtube.download";

    fn subscription() -> SubscriptionSpec {
        SubscriptionSpec {
            subscriber_id: Self::SUBSCRIBER_ID,
            inputs: vec![InputSpec {
                event_type: YoutubeUrlRequested::EVENT_TYPE,
                queue_kind: QueueKind::FifoDropOldest { capacity: 4 },
            }],
        }
    }

    async fn handle(&mut self, event: Arc<EnrichedEvent>, bus: &EventBus) -> anyhow::Result<()> {
        let req = expect::<YoutubeUrlRequested>(&event.event, YoutubeUrlRequested::EVENT_TYPE)?;
        let video_file_path = Self::download_video(&req.job.url, &req.job.cache_dir).await?;

        bus.publish(Arc::new(YoutubeVideoDownloaded::new(
            event.event.event_id(),
            req.job.clone(),
            video_file_path,
        )));

        Ok(())
    }
}
