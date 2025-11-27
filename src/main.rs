use std::{path::PathBuf, str::FromStr};

use thiserror::Error;
use tokio::process::Command;

struct Transcript {}

#[derive(Error, Debug)]
enum TranscriptError {
    #[error("Download failed for {url}. {reason}")]
    DownloadFailed { url: String, reason: String },
    #[error("Audio extraction failed for {video_path}. {reason}")]
    AudioExtractionFailed { video_path: PathBuf, reason: String },
    #[error("Transctiption failed for {audio_path}. {reason}")]
    TranscriptFailed { audio_path: PathBuf, reason: String },
    #[error("External tool not found - {tool}")]
    ExternalToolNotFound { tool: String },
    #[error("Unhandled io error. {0}")]
    IoError(std::io::Error),
}

async fn download_video(url: &str) -> Result<PathBuf, TranscriptError> {
    let path = PathBuf::from_str("{}.(ext)s", hashed_url);
    let mut ytdlp = Command::new("yt-dlp")
        .arg("--extractor-args \"youtube:player_client=android,web\"")
        .arg("-f 'best'")
        .arg(url)
        .arg("-o 'video.%(ext)s")
        .spawn()?;

    todo!()
}

async fn extract_audio(video_path: PathBuf) -> Result<PathBuf, TranscriptError> {
    todo!()
}

async fn transcribe_audio(audio_path: PathBuf) -> Result<Transcript, TranscriptError> {
    todo!()
}

async fn process_video(url: &str) -> Result<Transcript, TranscriptError> {
    let video_file = download_video(url).await?;
    let audio_file = extract_audio(video_file).await?;

    transcribe_audio(audio_file).await
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");
}
