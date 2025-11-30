use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    time::Duration,
};

use clap::{Parser, ValueEnum};
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

#[derive(Clone, Default, ValueEnum)]
enum Provider {
    #[default]
    Grok,
    Openai,
    Gemini,
}

struct ProviderConfig {
    api_url: &'static str,
    model: &'static str,
    env_var: &'static str,
}

impl Provider {
    fn config(&self) -> ProviderConfig {
        match self {
            Provider::Grok => ProviderConfig {
                api_url: "https://api.x.ai/v1/chat/completions",
                model: "grok-4-fast",
                env_var: "XAI_API_KEY",
            },
            Provider::Openai => ProviderConfig {
                api_url: "https://api.openai.com/v1/chat/completions",
                model: "gpt-5.1",
                env_var: "OPENAI_API_KEY",
            },
            Provider::Gemini => ProviderConfig {
                api_url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                model: "gemini-3-pro",
                env_var: "GEMINI_API_KEY",
            },
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Provider::Grok => "Grok",
            Provider::Openai => "OpenAI",
            Provider::Gemini => "Gemini",
        }
    }
}

fn get_cache_dir(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let url_hash = hasher.finish();

    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("bratishka")
        .join(url_hash.to_string())
}

fn find_video_in_cache(cache_dir: &Path) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if matches!(ext.as_str(), "mp4" | "webm" | "mkv" | "mov" | "avi") {
                return Some(path);
            }
        }
    }
    None
}

async fn download_video(url: &str, cache_dir: &Path) -> Result<PathBuf, TranscriptError> {
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
        return Err(TranscriptError::DownloadFailed {
            url: url.to_string(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    };

    let stdout_str = String::from_utf8_lossy(output.stdout.as_slice());
    let filepath = stdout_str.trim();

    let path = PathBuf::from(filepath);

    Ok(path)
}

async fn extract_audio(video_path: &Path, audio_path: &Path) -> Result<(), TranscriptError> {
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
        return Err(TranscriptError::AudioExtractionFailed {
            video_path: video_path.to_path_buf(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    };

    Ok(())
}

async fn transcribe_audio(
    audio_path: &Path,
    output_path: &Path,
) -> Result<Transcript, TranscriptError> {
    // Whisper outputs to same dir as input with .json extension
    // We need to use output_dir to control where it writes
    let output_dir = output_path.parent().unwrap_or(std::path::Path::new("."));

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
        return Err(TranscriptError::TranscriptFailed {
            audio_path: audio_path.to_path_buf(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    };

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
    provider: &Provider,
) -> Result<VideoReport, TranscriptError> {
    let config = provider.config();
    let api_key = std::env::var(config.env_var).expect("validated at startup");

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

    /// AI provider for report generation
    #[arg(short, long, default_value = "grok")]
    provider: Provider,

    /// Force re-processing even if cached files exist
    #[arg(short, long)]
    force: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Validate API key early
    let config = cli.provider.config();
    if std::env::var(config.env_var).is_err() {
        eprintln!(
            "{} {} environment variable is not set",
            style("Error:").red().bold(),
            config.env_var
        );
        std::process::exit(1);
    }

    let url = cli.url;

    // Setup cache directory
    let cache_dir = get_cache_dir(&url);
    fs::create_dir_all(&cache_dir).await?;

    println!(
        "\n{}  {}\n",
        style("bratishka").cyan().bold(),
        style("Video Analyzer").dim()
    );

    // Step 1: Download (check cache)
    let video_file = if !cli.force {
        if let Some(cached) = find_video_in_cache(&cache_dir) {
            println!(
                "{} Downloaded {}",
                style("✓").green().bold(),
                style("(cached)").dim()
            );
            cached
        } else {
            let spinner = create_spinner("Downloading video...");
            let video = download_video(&url, &cache_dir).await?;
            spinner.finish_with_message(format!(
                "{} Downloaded: {}",
                style("✓").green().bold(),
                style(video.file_name().unwrap().to_string_lossy()).dim()
            ));
            video
        }
    } else {
        let spinner = create_spinner("Downloading video...");
        let video = download_video(&url, &cache_dir).await?;
        spinner.finish_with_message(format!(
            "{} Downloaded: {}",
            style("✓").green().bold(),
            style(video.file_name().unwrap().to_string_lossy()).dim()
        ));
        video
    };

    // Step 2: Extract audio (check cache)
    let audio_file = cache_dir.join("audio.wav");
    if !cli.force && audio_file.exists() {
        println!(
            "{} Audio extracted {}",
            style("✓").green().bold(),
            style("(cached)").dim()
        );
    } else {
        let spinner = create_spinner("Extracting audio...");
        extract_audio(&video_file, &audio_file).await?;
        spinner.finish_with_message(format!("{} Audio extracted", style("✓").green().bold()));
    }

    // Step 3: Transcribe (check cache)
    let transcript_path = cache_dir.join("transcript.json");
    let transcript = if !cli.force && transcript_path.exists() {
        let json_content = fs::read_to_string(&transcript_path).await?;
        let transcript: Transcript = serde_json::from_str(&json_content)?;
        let duration_mins = transcript
            .segments
            .last()
            .map(|s| s.end / 60.0)
            .unwrap_or(0.0);
        println!(
            "{} Transcribed: {:.1} min, {} {}",
            style("✓").green().bold(),
            duration_mins,
            style(&transcript.language).yellow(),
            style("(cached)").dim()
        );
        transcript
    } else {
        let spinner = create_spinner("Transcribing with Whisper...");
        let transcript = transcribe_audio(&audio_file, &transcript_path).await?;
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
        transcript
    };

    // Step 4: Generate report (check cache with provider+lang)
    let report_lang = cli.lang.unwrap_or_else(|| transcript.language.clone());
    let provider_name = match cli.provider {
        Provider::Grok => "grok",
        Provider::Openai => "openai",
        Provider::Gemini => "gemini",
    };
    let report_filename = format!("report_{}_{}.json", provider_name, report_lang);
    let report_path = cache_dir.join(&report_filename);

    let report = if !cli.force && report_path.exists() {
        let json_content = fs::read_to_string(&report_path).await?;
        let report: VideoReport = serde_json::from_str(&json_content)?;
        println!(
            "{} Report generated ({}) {}",
            style("✓").green().bold(),
            cli.provider.name(),
            style("(cached)").dim()
        );
        report
    } else {
        let spinner = create_spinner(&format!(
            "Generating {} report with {}...",
            report_lang,
            cli.provider.name()
        ));
        let report = generate_report(transcript, &report_lang, &cli.provider).await?;
        // Save to cache
        let pretty_json = serde_json::to_string_pretty(&report)?;
        fs::write(&report_path, &pretty_json).await?;
        spinner.finish_with_message(format!(
            "{} Report generated ({})",
            style("✓").green().bold(),
            cli.provider.name()
        ));
        report
    };

    println!(
        "\n{} {}\n",
        style("Saved:").dim(),
        style(report_path.display()).cyan()
    );
    println!("{}", style("─".repeat(60)).dim());

    // Human-readable output
    let readable = format_report_readable(&report);
    println!("{}", readable);

    Ok(())
}
