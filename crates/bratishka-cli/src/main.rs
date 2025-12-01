use std::time::Duration;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::fs;

use bratishka_core::{
    Provider, download_video, extract_audio, find_video_in_cache, format_report_readable,
    generate_report, get_audio_path, get_cache_dir, get_report_path, get_transcript_path,
    load_report, load_transcript, save_report, transcribe_audio,
};

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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let provider: Provider = cli.provider.into();

    // Validate API key early
    if let Err(e) = provider.validate_api_key() {
        eprintln!("{} {}", style("Error:").red().bold(), e);
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
        spinner.finish_with_message(format!("{} Audio extracted", style("✓").green().bold()));
    }

    // Step 3: Transcribe (check cache)
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
            "{} Report generated ({})",
            style("✓").green().bold(),
            provider.name()
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
