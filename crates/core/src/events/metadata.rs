use uuid::Uuid;

#[derive(Clone)]
pub struct EventMetadata {
    pub event_id: Uuid,
    pub event_type: &'static str,
    pub timestamp_micros: i64,
    pub ingest_seq: u64,
    pub session_id: Uuid,

    pub parent_ids: [Option<Uuid>; 4],
    pub parent_count: u8,
}
