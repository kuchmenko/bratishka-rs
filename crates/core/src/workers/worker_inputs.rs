use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{
    events::EnrichedEvent,
    queues::{FifoDropOldestReceiver, Latest1Queue},
};

pub struct Latest1Input {
    pub event_type: &'static str,
    pub queue: Arc<Latest1Queue<Arc<EnrichedEvent>>>,
}

pub enum FifoReceiver {
    FifoDropOldest(FifoDropOldestReceiver<Arc<EnrichedEvent>>),
    Isolated(mpsc::Receiver<Arc<EnrichedEvent>>),
}

pub struct FifoInput {
    pub event_type: &'static str,
    pub receiver: FifoReceiver,
}

pub struct WorkerInputs {
    pub latest: Vec<Latest1Input>,
    pub fifos: Vec<FifoInput>,
    pub notify_any: Arc<tokio::sync::Notify>,
    pub fifo_index: usize,
}

pub enum WorkerBatch {
    Snapshots(Vec<SnapshotUpdate>),
    FifoItem {
        event_type: &'static str,
        event: Arc<EnrichedEvent>,
    },
}

pub struct SnapshotUpdate {
    pub event_type: &'static str,
    pub event: Arc<EnrichedEvent>,
}

impl WorkerInputs {
    pub async fn next(&mut self) -> WorkerBatch {
        loop {
            let mut snaps = Vec::new();
            for l in &self.latest {
                if let Some(e) = l.queue.try_recv() {
                    snaps.push(SnapshotUpdate {
                        event_type: l.event_type,
                        event: e,
                    });
                }
            }

            if !snaps.is_empty() {
                return WorkerBatch::Snapshots(snaps);
            }

            if !self.fifos.is_empty() {
                let start = self.fifo_index;

                loop {
                    let i = self.fifo_index;
                    self.fifo_index = (self.fifo_index + 1) % self.fifos.len();
                    let fifo = &mut self.fifos[i];

                    let item = match fifo.receiver {
                        FifoReceiver::FifoDropOldest(ref mut r) => r.try_recv(),
                        FifoReceiver::Isolated(ref mut r) => r.try_recv().ok(),
                    };

                    if let Some(e) = item {
                        return WorkerBatch::FifoItem {
                            event_type: fifo.event_type,
                            event: e,
                        };
                    }

                    if self.fifo_index == start {
                        break;
                    }
                }
            }
            self.notify_any.notified().await;
        }
    }
}
