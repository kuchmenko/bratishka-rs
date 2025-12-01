use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BratishkaError {
    #[error("Download failed for {url}: {reason}")]
    DownloadFailed { url: String, reason: String },

    #[error("Audio extraction failed for {video_path}: {reason}")]
    AudioExtractionFailed { video_path: PathBuf, reason: String },

    #[error("Transcription failed for {audio_path}: {reason}")]
    TranscriptFailed { audio_path: PathBuf, reason: String },

    #[error("Report generation failed: {reason}")]
    ReportFailed { reason: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("API request failed: {0}")]
    ApiError(#[from] reqwest::Error),

    #[error("Missing API key: {env_var} environment variable is not set")]
    MissingApiKey { env_var: String },
}

pub type Result<T> = std::result::Result<T, BratishkaError>;
