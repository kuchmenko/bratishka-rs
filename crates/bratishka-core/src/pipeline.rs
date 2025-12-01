use std::path::{Path, PathBuf};

use tokio::{fs, process::Command};

use crate::{
    error::{BratishkaError, Result},
    format::format_transcript_with_timestamps,
    provider::Provider,
    types::{Transcript, VideoReport},
};

/// Download a video from URL using yt-dlp
pub async fn download_video(url: &str, cache_dir: &Path) -> Result<PathBuf> {
    let output_template = cache_dir.join("video.%(ext)s");
    let output = Command::new("yt-dlp")
        .arg(url)
        .arg("--print")
        .arg("after_move:filepath")
        .arg("--extractor-args")
        .arg("youtube:player_client=android,web")
        .arg("-f")
        .arg("best")
        .arg("-o")
        .arg(&output_template)
        .output()
        .await?;

    if !output.status.success() {
        return Err(BratishkaError::DownloadFailed {
            url: url.to_string(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let stdout_str = String::from_utf8_lossy(output.stdout.as_slice());
    let filepath = stdout_str.trim();
    Ok(PathBuf::from(filepath))
}

/// Extract audio from video using ffmpeg
pub async fn extract_audio(video_path: &Path, audio_path: &Path) -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-vn")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg(audio_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(BratishkaError::AudioExtractionFailed {
            video_path: video_path.to_path_buf(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// Transcribe audio using Whisper
pub async fn transcribe_audio(audio_path: &Path, output_path: &Path) -> Result<Transcript> {
    let output_dir = output_path.parent().unwrap_or(Path::new("."));

    let output = Command::new("whisper")
        .arg(audio_path)
        .arg("--model")
        .arg("base")
        .arg("--output_format")
        .arg("json")
        .arg("--output_dir")
        .arg(output_dir)
        .output()
        .await?;

    if !output.status.success() {
        return Err(BratishkaError::TranscriptFailed {
            audio_path: audio_path.to_path_buf(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    // Whisper names output based on input filename
    let whisper_output = output_dir.join("audio.json");

    // Rename to our expected path if different
    if whisper_output != output_path {
        fs::rename(&whisper_output, output_path).await?;
    }

    let json_content = fs::read_to_string(output_path).await?;
    let transcript: Transcript = serde_json::from_str(&json_content)?;

    Ok(transcript)
}

/// Load a transcript from a cached file
pub async fn load_transcript(path: &Path) -> Result<Transcript> {
    let json_content = fs::read_to_string(path).await?;
    let transcript: Transcript = serde_json::from_str(&json_content)?;
    Ok(transcript)
}

/// Generate a report using an AI provider
pub async fn generate_report(
    transcript: &Transcript,
    provider: &Provider,
    report_lang: &str,
) -> Result<VideoReport> {
    let config = provider.config();
    let api_key = provider.validate_api_key()?;

    let duration_seconds = transcript.segments.last().map(|s| s.end).unwrap_or(0.0);
    let duration_minutes = duration_seconds / 60.0;

    let formatted_transcript = format_transcript_with_timestamps(transcript);

    let system_prompt = format!(
        r#"You are a video content analyzer. Your task is to analyze video transcripts and generate structured reports.

IMPORTANT: Write ALL text content (title, summary, topics, takeaways, chapter titles/summaries, prerequisites, target_audience) in {lang} language.

You MUST output ONLY valid JSON matching this exact structure (no markdown, no explanation):
{{
  "title": "Descriptive title for the video",
  "summary": "2-3 sentence summary of the entire video content",
  "duration_minutes": <number>,
  "language": "{lang}",
  "difficulty": "Beginner|Intermediate|Advanced",
  "topics": ["topic1", "topic2", "topic3"],
  "key_takeaways": ["takeaway1", "takeaway2", "takeaway3", "takeaway4", "takeaway5"],
  "chapters": [
    {{"start_seconds": 0, "end_seconds": 180, "title": "Chapter title", "summary": "1-2 sentence chapter summary"}}
  ],
  "prerequisites": ["prerequisite1", "prerequisite2"],
  "target_audience": "Description of who this video is for"
}}

Rules:
- Identify 5-10 logical chapters based on topic changes
- Use actual timestamps from the transcript for chapter boundaries
- Key takeaways should be actionable insights (5-7 items)
- Topics should be technical concepts covered (3-7 items)
- Output ONLY the JSON, nothing else"#,
        lang = report_lang
    );

    let user_prompt = format!(
        "Analyze this video transcript (duration: {:.1} minutes, language: {}):\n\n{}",
        duration_minutes, transcript.language, formatted_transcript
    );

    let response = reqwest::Client::new()
        .post(config.api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": config.model,
            "messages": [
                {
                    "role": "system",
                    "content": &system_prompt,
                },
                {
                    "role": "user",
                    "content": user_prompt,
                },
            ],
            "temperature": 0.3,
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    // Extract content from response
    let content = response["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| BratishkaError::ReportFailed {
            reason: format!("Invalid API response: {:?}", response),
        })?;

    // Parse JSON content into VideoReport
    let report: VideoReport = serde_json::from_str(content)?;

    Ok(report)
}

/// Load a report from a cached file
pub async fn load_report(path: &Path) -> Result<VideoReport> {
    let json_content = fs::read_to_string(path).await?;
    let report: VideoReport = serde_json::from_str(&json_content)?;
    Ok(report)
}

/// Save a report to a file
pub async fn save_report(report: &VideoReport, path: &Path) -> Result<()> {
    let pretty_json = serde_json::to_string_pretty(report)?;
    fs::write(path, &pretty_json).await?;
    Ok(())
}
