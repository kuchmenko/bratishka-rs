use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicU64},
};

use crate::{
    events::EnrichedEvent,
    queues::{FifoDropOldestQueue, IsolatedForwarder, Latest1Queue},
};

pub struct Routes {
    pub table: HashMap<&'static str, Vec<Route>>,
}

pub struct Route {
    pub subscriber_id: &'static str,
    pub inbox: RouteInbox,
    pub drops_total: Arc<AtomicU64>,
}

pub enum RouteInbox {
    Latest1(Arc<Latest1Queue<Arc<EnrichedEvent>>>),
    FifoDropOldest(Arc<FifoDropOldestQueue<Arc<EnrichedEvent>>>),
    Isolated(IsolatedForwarder<Arc<EnrichedEvent>>),
}

impl RouteInbox {
    pub fn try_deliver(&self, event: Arc<EnrichedEvent>) -> bool {
        match self {
            RouteInbox::Latest1(q) => {
                q.set(event);
                true
            }
            RouteInbox::FifoDropOldest(q) => {
                q.push_overwrite(event);
                true
            }
            RouteInbox::Isolated(fwd) => fwd.try_send(event).is_ok(),
        }
    }
}
