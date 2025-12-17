use bratishka_core::events::Event;

use crate::workers::events::{EventHeader, JobSpec};

#[derive(serde::Serialize)]
pub struct YoutubeAudioExtracted {
    pub header: EventHeader,
    pub job: JobSpec,
    pub audio_file_path: std::path::PathBuf,
}

impl YoutubeAudioExtracted {
    pub const EVENT_TYPE: &'static str = "youtube.audio_extracted";

    pub fn new(
        parent_event_id: uuid::Uuid,
        job: JobSpec,
        audio_file_path: std::path::PathBuf,
    ) -> Self {
        Self {
            header: EventHeader {
                event_id: uuid::Uuid::new_v4(),
                parent_ids: vec![parent_event_id],
                timestamp: std::time::SystemTime::now(),
            },
            job,
            audio_file_path,
        }
    }
}

impl Event for YoutubeAudioExtracted {
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
