use std::path::{Path, PathBuf};

use tokio::{fs, process::Command};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    cache::get_model_dir,
    error::{BratishkaError, Result},
    inteligence::analyze_sections,
    provider::Provider,
    types::{Segment, Transcript, VideoReport},
};

pub const MODEL_NAME: &str = "ggml-medium-q5_0.bin";

pub async fn ensure_model(cache_dir: &Path) -> Result<PathBuf> {
    let download_url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        MODEL_NAME
    );
    let model_dir = get_model_dir(cache_dir);

    if !model_dir.exists() {
        fs::create_dir_all(&model_dir).await?;
    }

    let model_path = model_dir.join(MODEL_NAME);
    if !model_path.exists() {
        let output = Command::new("curl")
            .arg("-L")
            .arg(&download_url)
            .arg("-o")
            .arg(&model_path)
            .output()
            .await?;

        if !output.status.success() {
            return Err(BratishkaError::ModelDownloadFailed {
                url: download_url.to_string(),
                reason: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
    }

    Ok(model_path)
}

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

/// Transcribe audio using whisper-rs with passed model
pub async fn transcribe_audio(
    audio_path: &Path,
    output_path: &Path,
    model_path: &str,
) -> Result<Transcript> {
    let mut reader = hound::WavReader::open(audio_path).unwrap();
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / i16::MAX as f32)
        .collect();

    // load a context and model
    let mut ctx_params = WhisperContextParameters {
        use_gpu: true,
        flash_attn: true,
        ..Default::default()
    };
    ctx_params.flash_attn = true;
    let ctx =
        WhisperContext::new_with_params(model_path, ctx_params).expect("failed to load model");

    // create a params object
    let params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });

    // now we can run the model
    let mut state = ctx.create_state().expect("failed to create state");
    state.full(params, &samples).expect("failed to run model");

    let mut text = String::new();
    let mut segments: Vec<Segment> = Vec::new();

    for segment in state.as_iter() {
        let seg_text = match segment.to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let seg = Segment {
            start: segment.start_timestamp() as f64 / 100.0,
            end: segment.end_timestamp() as f64 / 100.0,
            text: seg_text.to_string(),
        };
        segments.push(seg);

        text.push_str(seg_text);
    }

    let language_index = state.full_lang_id_from_state();
    let language = whisper_rs::get_lang_str(language_index);

    let transcript = Transcript {
        language: language.unwrap_or("Unknown").to_string(),
        segments,
        text,
    };

    fs::write(output_path, serde_json::to_string_pretty(&transcript)?).await?;

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

    let sections = analyze_sections(provider, transcript).await?;

    let system_prompt = format!(
        r#"You are a report compiler with web search access. Synthesize pre-analyzed sections into a comprehensive, easy-to-read report.

  IMPORTANT: Write ALL content in {lang} language.

  INPUT: Pre-analyzed sections with summaries, key_concepts, and external_context

  YOUR TASK:
  1. Find connections and cross-references between sections
  2. Use web search to fill knowledge gaps or add context
  3. Rewrite section summaries to be clearer and more connected
  4. Extract actionable takeaways from all available information
  5. Assess cognitive difficulty (how hard to understand, not technical complexity)

  OUTPUT: Return ONLY valid JSON:
  {{
    "title": "Clear, descriptive video title",
    "summary": "3-4 sentences explaining what viewer will learn and why it matters",
    "duration_minutes": <number>,
    "language": "{lang}",
    "difficulty": "Easy to understand|Moderate cognitive load|Cognitively demanding",
    "key_takeaways": [
      "Actionable insight 1 (what viewer should do/remember)",
      "Actionable insight 2",
      "..."
    ],
    "sections": [
      {{
        "start_seconds": 0.0,
        "end_seconds": 180.0,
        "title": "Section title",
        "summary": "Refined summary with cross-references and enriched context. Example: 'This builds on the concept from Section 2...'"
      }}
    ]
  }}

  RULES:
  - Cross-reference related concepts across sections in summaries
  - Use web search when you need to clarify complex terms or add context
  - Key takeaways = 5-7 actionable insights (what to DO, not just what was said)
  - Difficulty based on: concept density, abstraction level, prerequisite knowledge needed
  - Rewrite section summaries to be self-contained but connected
  - Focus on making content easy to understand and retain
  - Output ONLY JSON, nothing else"#,
        lang = report_lang
    );

    let prepared_sections = serde_json::to_string_pretty(&sections)?;

    let user_prompt = format!(
        "Analyze this video transcript (duration: {:.1} minutes, language: {}):\n\n{}",
        duration_minutes, transcript.language, prepared_sections
    );

    let response = reqwest::Client::new()
        .post(config.api_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": config.model,
                    "tools": [{"type": "web_search"}],
            "input": [
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

    // Extract content from response - /v1/responses format
    let content = response["output"]
        .as_array()
        .and_then(|arr| arr.iter().rev().find(|item| item["type"] == "message"))
        .and_then(|msg| msg["content"][0]["text"].as_str())
        .ok_or_else(|| BratishkaError::ReportFailed {
            reason: format!("Invalid API response structure: {:?}", response),
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
