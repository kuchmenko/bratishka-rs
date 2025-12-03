//! Bratishka Core Library
//!
//! Core functionality for downloading YouTube videos, transcribing with Whisper,
//! and generating AI-powered reports.

pub mod cache;
pub mod error;
pub mod format;
pub mod pipeline;
pub mod provider;
pub mod types;

// Re-export commonly used items at crate root
pub use cache::{
    find_video_in_cache, get_audio_path, get_cache_dir, get_report_path, get_transcript_path,
};
pub use error::{BratishkaError, Result};
pub use format::{format_report_readable, format_timestamp, format_transcript_with_timestamps};
pub use pipeline::{
    download_video, extract_audio, generate_report, load_report, load_transcript, save_report,
    transcribe_audio,
};
pub use provider::{Provider, ProviderConfig};
pub use types::{Chapter, Segment, Transcript, VideoReport};
