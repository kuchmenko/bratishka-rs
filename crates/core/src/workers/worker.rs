use std::sync::Arc;

use anyhow::{Ok, Result};
use tokio::sync::broadcast;

use crate::{
    events::{EnrichedEvent, EventBus},
    workers::{PipelineFailed, SubscriptionSpec, WorkerBatch, WorkerInputs},
};

pub trait Worker: Send + Sized + 'static {
    const SUBSCRIBER_ID: &'static str;
    fn subscription() -> SubscriptionSpec;
    async fn handle(&mut self, event: Arc<EnrichedEvent>, bus: &EventBus) -> Result<()>;
    async fn run(
        mut self,
        mut inputs: WorkerInputs,
        bus: Arc<EventBus>,
        mut shutdown: broadcast::Receiver<()>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                _ = shutdown.recv() => return Ok(()),
                batch = inputs.next() => match batch {
                    WorkerBatch::Snapshots(_snapshot_updates) => todo!(),
                    WorkerBatch::FifoItem { event_type: _event_type, event } => {
                        let parent = Arc::clone(&event);
                        if let Err(e) = self.handle(event, &bus).await {
                            bus.publish(Arc::new(PipelineFailed::new(Arc::clone(&parent.event), Self::SUBSCRIBER_ID, format!("{e}"))));
                        }

                    },
                }
            }
        }
    }
}
