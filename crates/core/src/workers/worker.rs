use std::sync::Arc;

use anyhow::Result;
use tokio::sync::broadcast;

use crate::{
    events::{EnrichedEvent, EventBus},
    workers::{PipelineFailed, SubscriptionSpec, WorkerBatch, WorkerInputs},
};

pub trait Worker: Send + 'static {
    const SUBSCRIBER_ID: &'static str;
    fn subscription() -> SubscriptionSpec;
    async fn handle(&mut self, event: Arc<EnrichedEvent>, bus: &EventBus) -> Result<()>;
}

pub async fn run_worker<W: Worker>(
    mut worker: W,
    mut inputs: WorkerInputs,
    bus: EventBus,
    mut shutdown: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = shutdown.recv() => return,
            batch = inputs.next() => match batch {
                WorkerBatch::Snapshots(_snapshot_updates) => todo!(),
                WorkerBatch::FifoItem { event_type: _event_type, event } => {
                    let parent = Arc::clone(&event);
                    if let Err(e) = worker.handle(event, &bus).await {
                        bus.publish(Arc::new(PipelineFailed::new(Arc::clone(&parent.event), W::SUBSCRIBER_ID, format!("{e}"))));
                    }

                },
            }
        }
    }
}
