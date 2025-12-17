use bratishka_core::events::Event;

use crate::{
    types::Transcript,
    workers::events::{EventHeader, JobSpec},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SourceSection {
    pub name: String,
    pub content: String,
    pub started_at: f64,
    pub ended_at: f64,
    pub key_concepts: Vec<String>,
    pub external_context: String,
    pub summary: String,
}

#[derive(serde::Serialize)]
pub struct SectionsAnalyzed {
    pub header: EventHeader,
    pub job: JobSpec,
    pub sections: Vec<SourceSection>,
    pub transcript: Transcript,
}

impl SectionsAnalyzed {
    pub const EVENT_TYPE: &'static str = "sections.analyzed";

    pub fn new(
        parent_event_id: uuid::Uuid,
        job: JobSpec,
        sections: Vec<SourceSection>,
        transcript: Transcript,
    ) -> Self {
        Self {
            header: EventHeader {
                event_id: uuid::Uuid::new_v4(),
                parent_ids: vec![parent_event_id],
                timestamp: std::time::SystemTime::now(),
            },
            job,
            sections,
            transcript,
        }
    }
}

impl Event for SectionsAnalyzed {
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
