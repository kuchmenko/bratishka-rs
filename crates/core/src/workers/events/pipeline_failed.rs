use std::{any::Any, sync::Arc, time::SystemTime};

use serde::Serialize;
use uuid::Uuid;

use crate::events::Event;

#[derive(Serialize)]
pub struct PipelineFailed {
    pub event_id: Uuid,
    pub ts: SystemTime,
    pub parents: [Uuid; 1],
    pub stage: &'static str,
    pub message: String,
}

impl PipelineFailed {
    pub const EVENT_TYPE: &'static str = "pipeline.failed";

    pub fn new(event: Arc<dyn Event>, subscriber_id: &'static str, message: String) -> Self {
        Self {
            message,
            event_id: event.event_id(),
            ts: SystemTime::now(),
            parents: [event.event_id()],
            stage: subscriber_id,
        }
    }
}

impl Event for PipelineFailed {
    fn event_id(&self) -> uuid::Uuid {
        self.event_id
    }

    fn parent_ids(&self) -> &[uuid::Uuid] {
        &self.parents
    }

    fn event_type(&self) -> &'static str {
        PipelineFailed::TYPE
    }

    fn timestamp(&self) -> std::time::SystemTime {
        self.ts
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self as &dyn Any
    }
}
