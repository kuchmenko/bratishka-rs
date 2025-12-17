use bratishka_core::events::Event;

use crate::workers::events::{EventHeader, JobSpec};

#[derive(serde::Serialize)]
pub struct YoutubeVideoDownloaded {
    pub header: EventHeader,
    pub job: JobSpec,
    pub video_file_path: std::path::PathBuf,
}

impl YoutubeVideoDownloaded {
    pub const EVENT_TYPE: &'static str = "youtube.video_downloaded";

    pub fn new(
        parent_event_id: uuid::Uuid,
        job: JobSpec,
        video_file_path: std::path::PathBuf,
    ) -> Self {
        Self {
            header: EventHeader {
                event_id: uuid::Uuid::new_v4(),
                parent_ids: vec![parent_event_id],
                timestamp: std::time::SystemTime::now(),
            },
            job,
            video_file_path,
        }
    }
}

impl Event for YoutubeVideoDownloaded {
    fn event_id(&self) -> uuid::Uuid {
        self.header.event_id
    }

    fn parent_ids(&self) -> &[uuid::Uuid] {
        &self.header.parent_ids
    }

    fn event_type(&self) -> &'static str {
        Self::EVENT_TYPE
    }

    fn timestamp(&self) -> std::time::SystemTime {
        self.header.timestamp
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn std::any::Any
    }
}
