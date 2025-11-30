use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    time::Duration,
};

use clap::Parser;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, process::Command};

#[derive(Debug, Serialize, Deserialize)]
struct Transcript {
    text: String,
    segments: Vec<Segment>,
    language: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Segment {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Error, Debug)]
enum TranscriptError {
    #[error("Download failed for {url}. {reason}")]
    DownloadFailed { url: String, reason: String },
    #[error("Audio extraction failed for {video_path}. {reason}")]
    AudioExtractionFailed { video_path: PathBuf, reason: String },
    #[error("Transctiption failed for {audio_path}. {reason}")]
    TranscriptFailed { audio_path: PathBuf, reason: String },
    #[error("Unhandled io error. {0}")]
    IoError(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("API request failed: {0}")]
    ApiError(#[from] reqwest::Error),
}

#[derive(Debug, Serialize, Deserialize)]
struct VideoReport {
    title: String,
    summary: String,
    duration_minutes: f64,
    language: String,
    difficulty: String,

    topics: Vec<String>,
    key_takeaways: Vec<String>,

    chapters: Vec<Chapter>,

    prerequisites: Vec<String>,
    target_audience: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Chapter {
    start_seconds: f64,
    end_seconds: f64,
    title: String,
    summary: String,
}

async fn download_video(url: &str) -> Result<PathBuf, TranscriptError> {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hashed_url = hasher.finish();
    let output_template = format!("{}.%(ext)s", hashed_url);
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
        return Err(TranscriptError::DownloadFailed {
            url: url.to_string(),
            reason: "Video file download failed".to_string(),
        });
    };

    let stdout_str = String::from_utf8_lossy(output.stdout.as_slice());
    let filepath = stdout_str.trim();

    let path = PathBuf::from(filepath);

    Ok(path)
}

async fn extract_audio(video_path: PathBuf) -> Result<PathBuf, TranscriptError> {
    let audio_path = video_path.with_extension("wav");
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&video_path)
        .arg("-vn")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg("16000")
        .arg("-ac")
        .arg("1")
        .arg(&audio_path)
        .output()
        .await?;

    if !output.status.success() {
        return Err(TranscriptError::AudioExtractionFailed {
            video_path,
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    };

    Ok(audio_path)
}

async fn transcribe_audio(audio_path: PathBuf) -> Result<Transcript, TranscriptError> {
    let json_path = audio_path.with_extension("json");

    let output = Command::new("whisper")
        .arg(&audio_path)
        .arg("--model")
        .arg("base")
        .arg("--output_format")
        .arg("json")
        .output()
        .await?;

    if !output.status.success() {
        return Err(TranscriptError::TranscriptFailed {
            audio_path,
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    };

    let json_content = fs::read_to_string(&json_path).await?;
    let transcript: Transcript = serde_json::from_str(&json_content)?;

    Ok(transcript)
}

fn format_timestamp(seconds: f64) -> String {
    let mins = (seconds / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    format!("{:02}:{:02}", mins, secs)
}

fn format_transcript_with_timestamps(transcript: &Transcript) -> String {
    transcript
        .segments
        .iter()
        .map(|seg| format!("[{}] {}", format_timestamp(seg.start), seg.text.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn generate_report(
    transcript: Transcript,
    report_lang: &str,
) -> Result<VideoReport, TranscriptError> {
    let xai_api_key = std::env::var("XAI_API_KEY").expect("XAI_API_KEY is not set");

    let duration_seconds = transcript.segments.last().map(|s| s.end).unwrap_or(0.0);
    let duration_minutes = duration_seconds / 60.0;

    let formatted_transcript = format_transcript_with_timestamps(&transcript);

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
        .post("https://api.x.ai/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", xai_api_key))
        .json(&serde_json::json!({
            "model": "grok-4-fast",
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
        .ok_or_else(|| TranscriptError::TranscriptFailed {
            audio_path: PathBuf::from("api"),
            reason: format!("Invalid API response: {:?}", response),
        })?;

    // Parse JSON content into VideoReport
    let report: VideoReport = serde_json::from_str(content)?;

    Ok(report)
}

fn format_report_readable(report: &VideoReport) -> String {
    let mut output = String::new();

    // Title
    output.push_str(&format!("# {}\n\n", report.title));

    // Meta info
    output.push_str(&format!(
        "**Duration:** {:.0} minutes | **Difficulty:** {} | **Language:** {}\n\n",
        report.duration_minutes, report.difficulty, report.language
    ));

    // Summary
    output.push_str("## Summary\n\n");
    output.push_str(&report.summary);
    output.push_str("\n\n");

    // Topics
    output.push_str("## Topics Covered\n\n");
    for topic in &report.topics {
        output.push_str(&format!("• {}\n", topic));
    }
    output.push('\n');

    // Key Takeaways
    output.push_str("## Key Takeaways\n\n");
    for (i, takeaway) in report.key_takeaways.iter().enumerate() {
        output.push_str(&format!("{}. {}\n", i + 1, takeaway));
    }
    output.push('\n');

    // Chapters
    output.push_str("## Chapters\n\n");
    for chapter in &report.chapters {
        let start = format_timestamp(chapter.start_seconds);
        let end = format_timestamp(chapter.end_seconds);
        output.push_str(&format!("### [{}–{}] {}\n\n", start, end, chapter.title));
        output.push_str(&format!("{}\n\n", chapter.summary));
    }

    // Prerequisites
    if !report.prerequisites.is_empty() {
        output.push_str("## Prerequisites\n\n");
        for prereq in &report.prerequisites {
            output.push_str(&format!("• {}\n", prereq));
        }
        output.push('\n');
    }

    // Target Audience
    output.push_str("## Target Audience\n\n");
    output.push_str(&report.target_audience);
    output.push('\n');

    output
}

fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

#[derive(Parser)]
struct Cli {
    /// Video URL
    url: String,

    /// Report language (e.g., "en", "ru", "uk"). Defaults to video's detected language.
    #[arg(short, long)]
    lang: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let url = cli.url;

    println!(
        "\n{}  {}\n",
        style("bratishka").cyan().bold(),
        style("Video Analyzer").dim()
    );

    // Step 1: Download
    let spinner = create_spinner("Downloading video...");
    let video_file = download_video(&url).await?;
    spinner.finish_with_message(format!(
        "{} Downloaded: {}",
        style("✓").green().bold(),
        style(video_file.file_name().unwrap().to_string_lossy()).dim()
    ));

    // Step 2: Extract audio
    let spinner = create_spinner("Extracting audio...");
    let audio_file = extract_audio(video_file.clone()).await?;
    spinner.finish_with_message(format!("{} Audio extracted", style("✓").green().bold()));

    // Step 3: Transcribe
    let spinner = create_spinner("Transcribing with Whisper...");
    let transcript = transcribe_audio(audio_file).await?;
    let duration_mins = transcript
        .segments
        .last()
        .map(|s| s.end / 60.0)
        .unwrap_or(0.0);
    spinner.finish_with_message(format!(
        "{} Transcribed: {:.1} min, {} detected",
        style("✓").green().bold(),
        duration_mins,
        style(&transcript.language).yellow()
    ));

    // Step 4: Generate report
    let report_lang = cli.lang.unwrap_or_else(|| transcript.language.clone());
    let spinner = create_spinner(&format!("Generating {} report with AI...", report_lang));
    let report = generate_report(transcript, &report_lang).await?;
    spinner.finish_with_message(format!("{} Report generated", style("✓").green().bold()));

    // Save JSON
    let json_path = video_file.with_extension("report.json");
    let pretty_json = serde_json::to_string_pretty(&report)?;
    fs::write(&json_path, &pretty_json).await?;

    println!(
        "\n{} {}\n",
        style("Saved:").dim(),
        style(json_path.display()).cyan()
    );
    println!("{}", style("─".repeat(60)).dim());

    // Human-readable output
    let readable = format_report_readable(&report);
    println!("{}", readable);

    Ok(())
}
