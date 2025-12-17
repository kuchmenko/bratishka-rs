use bratishka_core::events::Event;

use crate::{
    types::Transcript,
    workers::events::{EventHeader, JobSpec},
};

#[derive(Clone, serde::Serialize)]
pub struct AudioTranscribed {
    pub header: EventHeader,
    pub job: JobSpec,
    pub transcript: Transcript,
}

impl AudioTranscribed {
    pub const EVENT_TYPE: &'static str = "audio.transcribed";

    pub fn new(parent_event_id: uuid::Uuid, job: JobSpec, transcript: Transcript) -> Self {
        Self {
            header: EventHeader {
                event_id: uuid::Uuid::new_v4(),
                parent_ids: vec![parent_event_id],
                timestamp: std::time::SystemTime::now(),
            },
            job,
            transcript,
        }
    }
}

impl Event for AudioTranscribed {
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
