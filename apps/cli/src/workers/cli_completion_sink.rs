use std::sync::Arc;

use bratishka_core::{
    events::{EnrichedEvent, EventBus, downcast_ref, expect},
    queues::QueueKind,
    workers::{InputSpec, PipelineFailed, SubscriptionSpec, Worker},
};
use tokio::sync::oneshot;

use crate::{types::VideoReport, workers::events::ReportCompiled};

pub struct CliCompletionSinkWorker {
    done: Option<oneshot::Sender<Result<VideoReport, PipelineFailed>>>,
}

impl CliCompletionSinkWorker {
    pub fn new(done: Option<oneshot::Sender<Result<VideoReport, PipelineFailed>>>) -> Self {
        Self { done }
    }
}

impl Worker for CliCompletionSinkWorker {
    const SUBSCRIBER_ID: &'static str = "cli.completion_sink";

    fn subscription() -> SubscriptionSpec {
        SubscriptionSpec {
            subscriber_id: Self::SUBSCRIBER_ID,
            inputs: vec![
                InputSpec {
                    event_type: ReportCompiled::EVENT_TYPE,
                    queue_kind: QueueKind::Isolated { output_buffer: 4 },
                },
                InputSpec {
                    event_type: PipelineFailed::EVENT_TYPE,
                    queue_kind: QueueKind::FifoDropOldest { capacity: 4 },
                },
            ],
        }
    }

    async fn handle(&mut self, event: Arc<EnrichedEvent>, bus: &EventBus) -> anyhow::Result<()> {
        if let Some(req) = downcast_ref::<ReportCompiled>(&event.event) {
            if let Some(done) = self.done.take() {
                done.send(Ok(req.report.clone()));
            }
        }

        if let Some(req) = downcast_ref::<PipelineFailed>(&event.event) {
            if let Some(done) = self.done.take() {
                done.send(Err(req.clone()));
            }
        }
        Ok(())
    }
}
