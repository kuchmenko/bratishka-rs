use std::collections::HashMap;

use crate::{queues::QueueKind, workers::WorkerInputs};

pub struct SubscriptionSpec {
    pub subscriber_id: &'static str,
    pub inputs: Vec<InputSpec>,
}

pub struct InputSpec {
    pub event_type: &'static str,
    pub queue_kind: QueueKind,
}

pub struct WorkerWiring {
    inputs: HashMap<&'static str, WorkerInputs>,
}

impl WorkerWiring {
    pub fn new(inputs: HashMap<&'static str, WorkerInputs>) -> Self {
        Self { inputs }
    }

    pub fn take(&mut self, subscriber_id: &'static str) -> Option<WorkerInputs> {
        self.inputs.remove(subscriber_id)
    }
}
