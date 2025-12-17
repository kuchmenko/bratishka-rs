pub mod audio_transcribed;
pub mod report_compiled;
pub mod sections_analyzed;
pub mod youtube_audio_extracted;
pub mod youtube_url_requested;
pub mod youtube_video_downloaded;

pub use audio_transcribed::*;
pub use report_compiled::*;
pub use sections_analyzed::*;
use std::time::SystemTime;
pub use youtube_audio_extracted::*;
pub use youtube_url_requested::*;
pub use youtube_video_downloaded::*;

#[derive(Clone, Debug, serde::Serialize)]
pub struct EventHeader {
    pub event_id: uuid::Uuid,
    pub parent_ids: Vec<uuid::Uuid>,
    pub timestamp: SystemTime,
}
