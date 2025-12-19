use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use bratishka_core::events::BusConfig;
use clap::{Parser, ValueEnum};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::fs;
use uuid::Uuid;

use crate::{
    pipeline::start_pipeline,
    provider::Provider,
    workers::events::{JobSpec, YoutubeUrlRequested},
};

mod cache;
mod error;
mod format;
mod inteligence;
mod pipeline;
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
    let args = Cli::parse();
    let job = JobSpec::from_cli(args).await?;
    println!("{} Checking model...", style("✓").green().bold());
    unsafe {
        whisper_rs::set_log_callback(Some(whisper_log_callback), std::ptr::null_mut());
    }

    println!("Starting pipeline...");
    let pipeline = start_pipeline(BusConfig {
        session_id: Uuid::new_v4(),
        strict_routing: false,
    })
    .await?;
    println!("Pipeline started");

    println!("Publishing job...");
    pipeline
        .bus
        .publish(Arc::new(YoutubeUrlRequested::new(job)));

    println!("Waiting for pipeline to finish...");

    match tokio::time::timeout(Duration::from_secs(30 * 60), pipeline.done_rx).await?? {
        Ok(done) => {
            println!("report saved at {}", done.display());
            Ok(())
        }
        Err(failed) => {
            eprintln!("pipeline failed at {}: {}", failed.stage, failed.message);
            let _ = pipeline.shutdown_tx.send(());
            Err(anyhow::anyhow!("pipeline failed"))
        }
    }
}
