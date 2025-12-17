use std::time::{Duration, Instant};

use anyhow::Result;
use clap::{Parser, ValueEnum};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::fs;

use crate::{
    cache::{
        find_video_in_cache, get_audio_path, get_cache_dir, get_report_path, get_root_cache_dir,
        get_transcript_path,
    },
    format::format_report_readable,
    pipeline_old::{
        download_video, ensure_model, extract_audio, generate_report, load_report, load_transcript,
        save_report, transcribe_audio,
    },
    provider::Provider,
};

mod cache;
mod error;
mod format;
mod inteligence;
mod pipeline_old;
mod provider;
mod types;
mod workers;

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        format!("{:.0}m {:.0}s", secs / 60.0, secs % 60.0)
    }
}

/// CLI wrapper for Provider enum (needed for clap ValueEnum)
#[derive(Clone, Default, ValueEnum)]
enum CliProvider {
    #[default]
    Grok,
    Openai,
    Gemini,
}

impl From<CliProvider> for Provider {
    fn from(cli: CliProvider) -> Self {
        match cli {
            CliProvider::Grok => Provider::Grok,
            CliProvider::Openai => Provider::Openai,
            CliProvider::Gemini => Provider::Gemini,
        }
    }
}

#[derive(Parser)]
#[command(name = "bratishka")]
#[command(
    about = "Download YouTube videos, transcribe with Whisper, and generate AI-powered reports"
)]
struct Cli {
    /// Video URL
    url: String,

    /// Report language (e.g., "en", "ru", "uk"). Defaults to video's detected language.
    #[arg(short, long)]
    lang: Option<String>,

    /// AI provider for report generation
    #[arg(short, long, default_value = "grok")]
    provider: CliProvider,

    /// Force re-processing even if cached files exist
    #[arg(short, long)]
    force: bool,
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

extern "C" fn whisper_log_callback(
    _level: u32,
    _message: *const std::ffi::c_char,
    _user_data: *mut std::ffi::c_void,
) {
    // silent
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let provider: Provider = cli.provider.into();

    unsafe {
        whisper_rs::set_log_callback(Some(whisper_log_callback), std::ptr::null_mut());
    }

    // Validate API key early
    if let Err(e) = provider.validate_api_key() {
        eprintln!("{} {}", style("Error:").red().bold(), e);
        std::process::exit(1);
    }

    let url = cli.url;

    // Setup cache directory
    let root_cache_dir = get_root_cache_dir();
    let cache_dir = get_cache_dir(&url);
    fs::create_dir_all(&cache_dir).await?;

    println!(
        "\n{}  {}\n",
        style("bratishka").cyan().bold(),
        style("Video Analyzer").dim()
    );

    // Ensure model is downloaded
    println!("{} Checking model...", style("✓").green().bold());
    let model_path = ensure_model(&root_cache_dir).await?;

    println!("{}", style("─".repeat(60)).dim());

    let total_start = Instant::now();

    // Step 1: Download (check cache)
    let step_start = Instant::now();
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
                "{} Downloaded: {} {}",
                style("✓").green().bold(),
                style(video.file_name().unwrap().to_string_lossy()).dim(),
                style(format!("[{}]", format_duration(step_start.elapsed()))).dim()
            ));
            video
        }
    } else {
        let spinner = create_spinner("Downloading video...");
        let video = download_video(&url, &cache_dir).await?;
        spinner.finish_with_message(format!(
            "{} Downloaded: {} {}",
            style("✓").green().bold(),
            style(video.file_name().unwrap().to_string_lossy()).dim(),
            style(format!("[{}]", format_duration(step_start.elapsed()))).dim()
        ));
        video
    };

    // Step 2: Extract audio (check cache)
    let step_start = Instant::now();
    let audio_file = get_audio_path(&cache_dir);
    if !cli.force && audio_file.exists() {
        println!(
            "{} Audio extracted {}",
            style("✓").green().bold(),
            style("(cached)").dim()
        );
    } else {
        let spinner = create_spinner("Extracting audio...");
        extract_audio(&video_file, &audio_file).await?;
        spinner.finish_with_message(format!(
            "{} Audio extracted {}",
            style("✓").green().bold(),
            style(format!("[{}]", format_duration(step_start.elapsed()))).dim()
        ));
    }

    // Step 3: Transcribe (check cache)
    let step_start = Instant::now();
    let transcript_path = get_transcript_path(&cache_dir);
    let transcript = if !cli.force && transcript_path.exists() {
        let transcript = load_transcript(&transcript_path).await?;
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
        let model_path_str = model_path.to_str().unwrap();
        let transcript = transcribe_audio(&audio_file, &transcript_path, model_path_str).await?;
        let duration_mins = transcript
            .segments
            .last()
            .map(|s| s.end / 60.0)
            .unwrap_or(0.0);
        spinner.finish_with_message(format!(
            "{} Transcribed: {:.1} min, {} {}",
            style("✓").green().bold(),
            duration_mins,
            style(&transcript.language).yellow(),
            style(format!("[{}]", format_duration(step_start.elapsed()))).dim()
        ));
        transcript
    };

    // Step 4: Generate report (check cache with provider+lang)
    let step_start = Instant::now();
    let report_lang = cli.lang.unwrap_or_else(|| transcript.language.clone());
    let report_path = get_report_path(&cache_dir, &provider, &report_lang);

    let report = if !cli.force && report_path.exists() {
        let report = load_report(&report_path).await?;
        println!(
            "{} Report generated ({}) {}",
            style("✓").green().bold(),
            provider.name(),
            style("(cached)").dim()
        );
        report
    } else {
        let spinner = create_spinner(&format!(
            "Generating {} report with {}...",
            report_lang,
            provider.name()
        ));
        let report = generate_report(&transcript, &provider, &report_lang).await?;
        // Save to cache
        save_report(&report, &report_path).await?;
        spinner.finish_with_message(format!(
            "{} Report generated ({}) {}",
            style("✓").green().bold(),
            provider.name(),
            style(format!("[{}]", format_duration(step_start.elapsed()))).dim()
        ));
        report
    };

    println!(
        "\n{} {}\n",
        style("Total time:").dim(),
        style(format_duration(total_start.elapsed())).cyan().bold()
    );

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
