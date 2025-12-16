use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::Result;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::{
    events::{EnrichedEvent, EventBus},
    queues::{FifoDropOldestQueue, IsolatedForwarder, Latest1Queue, QueueKind, StartupTasks},
    routes::{Route, RouteInbox, Routes},
    workers::{
        FifoInput, FifoReceiver, Latest1Input, SubscriptionSpec, WorkerInputs, WorkerWiring,
    },
};

pub struct BusConfig {
    pub session_id: Uuid,
    pub strict_routing: bool,
}

pub struct BusMetrics {
    pub unrouted_publish_total: AtomicU64,
}

impl BusMetrics {
    pub fn new() -> Self {
        Self {
            unrouted_publish_total: AtomicU64::new(0),
        }
    }

    pub fn record_unrouted(&self, _evt: &'static str) {
        self.unrouted_publish_total.fetch_add(1, Ordering::Relaxed);
    }
}

fn validate(subs: &[SubscriptionSpec]) -> Result<()> {
    use std::collections::HashSet;

    let mut seen_subscribers: HashSet<&'static str> = HashSet::new();
    for s in subs {
        if s.subscriber_id.trim().is_empty() {
            anyhow::bail!("empty subscriber_id");
        }
        if !seen_subscribers.insert(s.subscriber_id) {
            anyhow::bail!("duplicate subscriber_id={}", s.subscriber_id);
        }
        if s.inputs.is_empty() {
            anyhow::bail!("subscriber_id={} has no inputs", s.subscriber_id);
        }

        let mut seen_inputs: HashSet<&'static str> = HashSet::new();
        for i in &s.inputs {
            if i.event_type.trim().is_empty() {
                anyhow::bail!("subscriber_id={} has empty event_type", s.subscriber_id);
            }
            if !seen_inputs.insert(i.event_type) {
                anyhow::bail!(
                    "subscriber_id={} has duplicate input event_type={}",
                    s.subscriber_id,
                    i.event_type
                );
            }

            match i.queue_kind {
                QueueKind::Latest1 => {}
                QueueKind::FifoDropOldest { capacity } => {
                    anyhow::ensure!(capacity > 0, "capacity must be > 0")
                }
                QueueKind::BoundedDropNewest { capacity } => {
                    anyhow::ensure!(capacity > 0, "capacity must be > 0")
                }
                QueueKind::Isolated { output_buffer } => {
                    anyhow::ensure!(output_buffer > 0, "output_buffer must be > 0")
                }
            }
        }
    }
    Ok(())
}

pub struct EventBusBuilder {
    cfg: BusConfig,
    subs: Vec<SubscriptionSpec>,
}

impl EventBusBuilder {
    pub fn new(cfg: BusConfig) -> Self {
        Self {
            cfg,
            subs: Vec::new(),
        }
    }

    pub fn subscribe(mut self, s: SubscriptionSpec) -> Self {
        self.subs.push(s);
        self
    }

    pub fn build(self) -> Result<(EventBus, WorkerWiring, StartupTasks)> {
        validate(&self.subs)?;

        let mut routes: HashMap<&'static str, Vec<Route>> = HashMap::new();
        let mut wiring: HashMap<&'static str, WorkerInputs> = HashMap::new();
        let mut tasks = StartupTasks { tokio: Vec::new() };
        let metrics = Arc::new(BusMetrics::new());

        for spec in self.subs {
            let notify_any = Arc::new(Notify::new());
            let mut latest = Vec::new();
            let mut fifos = Vec::new();

            for input in spec.inputs {
                let drops_total = Arc::new(AtomicU64::new(0));

                match input.queue_kind {
                    QueueKind::Latest1 => {
                        let q = Arc::new(Latest1Queue::new(Arc::clone(&notify_any)));
                        routes.entry(input.event_type).or_default().push(Route {
                            subscriber_id: spec.subscriber_id,
                            inbox: RouteInbox::Latest1(Arc::clone(&q)),
                            drops_total: Arc::clone(&drops_total),
                        });
                        latest.push(Latest1Input {
                            event_type: input.event_type,
                            queue: q,
                        });
                    }
                    QueueKind::FifoDropOldest { capacity } => {
                        let q =
                            Arc::new(FifoDropOldestQueue::new(capacity, Arc::clone(&notify_any)));
                        routes.entry(input.event_type).or_default().push(Route {
                            subscriber_id: spec.subscriber_id,
                            inbox: RouteInbox::FifoDropOldest(Arc::clone(&q)),
                            drops_total: Arc::clone(&drops_total),
                        });
                        fifos.push(FifoInput {
                            event_type: input.event_type,
                            receiver: FifoReceiver::FifoDropOldest(q.receiver()),
                        });
                    }
                    QueueKind::BoundedDropNewest { capacity } => {
                        unimplemented!("Not need for now")
                    }
                    QueueKind::Isolated { output_buffer } => {
                        let (fwd, out_rx, drain_task) =
                            IsolatedForwarder::<Arc<EnrichedEvent>>::new(
                                output_buffer,
                                Arc::clone(&notify_any),
                            );
                        tasks.tokio.push(drain_task);

                        routes.entry(input.event_type).or_default().push(Route {
                            subscriber_id: spec.subscriber_id,
                            inbox: RouteInbox::Isolated(fwd),
                            drops_total: Arc::clone(&drops_total),
                        });

                        fifos.push(FifoInput {
                            event_type: input.event_type,
                            receiver: FifoReceiver::Isolated(out_rx),
                        });
                    }
                }
            }

            wiring.insert(
                spec.subscriber_id,
                WorkerInputs {
                    latest,
                    fifos,
                    notify_any,
                    fifo_index: 0,
                },
            );
        }

        let bus = EventBus::new(self.cfg, Routes { table: routes }, metrics);
        Ok((bus, WorkerWiring::new(wiring), tasks))
    }
}
