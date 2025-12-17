use std::{any::Any, sync::Arc, time::SystemTime};

use erased_serde::Serialize as ErasedSerialize;
use tokio::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persistence {
    None,
    Warm,
    Cold, // "must-persist" (best-effort)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotStorageClass {
    Full,
    MetadataOnly,
    Skip,
}

pub trait Event: Send + Sync + ErasedSerialize + 'static {
    fn event_id(&self) -> Uuid;
    fn parent_ids(&self) -> &[Uuid];
    fn event_type(&self) -> &'static str;
    fn timestamp(&self) -> SystemTime;

    fn schema_version(&self) -> u32 {
        1
    }

    fn persistence(&self) -> Persistence {
        Persistence::Warm
    }
    fn must_persist(&self) -> bool {
        self.persistence() == Persistence::Cold
    }
    fn hot_storage_class(&self) -> HotStorageClass {
        HotStorageClass::Full
    }

    fn indexable_text(&self) -> Option<&str> {
        None
    }

    fn as_any(&self) -> &dyn Any;
}

pub struct EnrichedEvent {
    pub event: Arc<dyn Event>,
    pub ingest_ns: u64,
    pub session_id: Uuid,
    pub ingested_at: Instant,
}

pub fn downcast_ref<T: 'static>(e: &Arc<dyn Event>) -> Option<&T> {
    e.as_any().downcast_ref::<T>()
}

pub fn expect<'a, T: 'static>(
    e: &'a Arc<dyn Event>,
    expected_event_type: &'static str,
) -> anyhow::Result<&'a T> {
    downcast_ref::<T>(e).ok_or_else(|| {
        anyhow::anyhow!(
            "expected event_type={}, got={}",
            expected_event_type,
            e.event_type()
        )
    })
}

pub trait EventExt {
    fn downcast_ref<T: Event>(&self) -> Option<&T>;
}

pub trait EventConcrete: Event {}

impl<T: Event> EventConcrete for T {}

impl EventExt for dyn Event {
    fn downcast_ref<T: Event>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}
