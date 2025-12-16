use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use tokio::time::Instant;
use uuid::Uuid;

use crate::{
    events::{BusConfig, BusMetrics, EnrichedEvent, Event},
    routes::Routes,
};

#[derive(Clone)]
pub struct EventBus {
    inner: Arc<EventBusInner>,
}

pub struct EventBusInner {
    session_id: Uuid,
    next_ingest_seq: AtomicU64,
    routes: Arc<Routes>,
    metrics: Arc<BusMetrics>,
    strict_routing: bool,
}

impl EventBus {
    pub fn new(cfg: BusConfig, routes: Routes, metrics: Arc<BusMetrics>) -> Self {
        Self {
            inner: Arc::new(EventBusInner {
                session_id: cfg.session_id,
                next_ingest_seq: AtomicU64::new(0),
                routes: Arc::new(routes),
                metrics,
                strict_routing: cfg.strict_routing,
            }),
        }
    }

    pub fn publish(&self, event: Arc<dyn Event>) {
        let ingest_ns = self.inner.next_ingest_seq.fetch_add(1, Ordering::Relaxed);

        let enriched_event = Arc::new(EnrichedEvent {
            event,
            session_id: self.inner.session_id,
            ingest_ns,
            ingested_at: Instant::now(),
        });

        let Some(routes) = self
            .inner
            .routes
            .table
            .get(enriched_event.event.event_type())
        else {
            self.inner
                .metrics
                .record_unrouted(enriched_event.event.event_type());

            if self.inner.strict_routing {
                panic!("Unrouted event type: {}", enriched_event.event.event_type());
            }

            return;
        };

        for route in routes {
            let ok = route.inbox.try_deliver(Arc::clone(&enriched_event));

            if !ok {
                route.drops_total.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn session_id(&self) -> Uuid {
        self.inner.session_id
    }
}
