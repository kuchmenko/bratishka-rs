use std::sync::Arc;

use bratishka_core::{
    events::{BusConfig, EventBus, EventBusBuilder, bus_builder},
    workers::{PipelineFailed, Worker},
};
use tokio::sync::{broadcast, oneshot};

use crate::{
    types::VideoReport,
    workers::{
        analyze_sections::AnalyzeSectionsWorker, cli_completion_sink::CliCompletionSinkWorker,
        compile_report::CompileReportWorker, download_video::DownloadVideoWorker,
        extract_audio::ExtractAudioWorker, transcribe_audio::TranscribeAudioWorker,
    },
};

pub struct PipelineHandle {
    pub bus: Arc<EventBus>,
    pub shutdown_tx: broadcast::Sender<()>,
    pub done_rx: oneshot::Receiver<Result<VideoReport, PipelineFailed>>,
}

pub async fn start_pipeline(bus_config: BusConfig) -> Result<PipelineHandle, anyhow::Error> {
    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);
    let (done_tx, done_rx) = oneshot::channel::<Result<VideoReport, PipelineFailed>>();
    let done_tx = Some(done_tx);

    println!("Building event bus...");
    let builder = EventBusBuilder::new(bus_config)
        .subscribe(DownloadVideoWorker::subscription())
        .subscribe(ExtractAudioWorker::subscription())
        .subscribe(TranscribeAudioWorker::subscription())
        .subscribe(AnalyzeSectionsWorker::subscription())
        .subscribe(CompileReportWorker::subscription())
        .subscribe(CliCompletionSinkWorker::subscription());

    println!("Builder is ready");

    let (bus, mut wiring, tasks) = builder.build()?;
    let arc_bus = Arc::new(bus);

    println!("Event bus is ready");

    println!("Starting drain tasks...");
    // start isolated drain tasks BEFORE sources publish anything
    for t in tasks.tokio {
        tokio::spawn(t);
    }

    println!("Drain tasks are ready");

    println!("Creating workers...");
    let download_worker = DownloadVideoWorker;
    let extract_audio_worker = ExtractAudioWorker;
    let transcribe_audio_worker = TranscribeAudioWorker;
    let analyze_sections_worker = AnalyzeSectionsWorker;
    let compile_report_worker = CompileReportWorker;
    let cli_completion_sink_worker = CliCompletionSinkWorker::new(done_tx);

    println!("Workers are ready");

    println!("Starting workers...");
    tokio::spawn(download_worker.run(
        wiring.take(DownloadVideoWorker::SUBSCRIBER_ID).unwrap(),
        arc_bus.clone(),
        shutdown_rx.resubscribe(),
    ));
    tokio::spawn(extract_audio_worker.run(
        wiring.take(ExtractAudioWorker::SUBSCRIBER_ID).unwrap(),
        arc_bus.clone(),
        shutdown_rx.resubscribe(),
    ));
    tokio::spawn(transcribe_audio_worker.run(
        wiring.take(TranscribeAudioWorker::SUBSCRIBER_ID).unwrap(),
        arc_bus.clone(),
        shutdown_rx.resubscribe(),
    ));
    tokio::spawn(analyze_sections_worker.run(
        wiring.take(AnalyzeSectionsWorker::SUBSCRIBER_ID).unwrap(),
        arc_bus.clone(),
        shutdown_rx.resubscribe(),
    ));
    tokio::spawn(compile_report_worker.run(
        wiring.take(CompileReportWorker::SUBSCRIBER_ID).unwrap(),
        arc_bus.clone(),
        shutdown_rx.resubscribe(),
    ));
    tokio::spawn(cli_completion_sink_worker.run(
        wiring.take(CliCompletionSinkWorker::SUBSCRIBER_ID).unwrap(),
        arc_bus.clone(),
        shutdown_rx.resubscribe(),
    ));
    println!("Workers are started");

    Ok(PipelineHandle {
        bus: arc_bus,
        shutdown_tx,
        done_rx,
    })
}
