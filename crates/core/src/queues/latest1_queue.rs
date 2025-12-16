use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

pub struct Latest1Queue<T> {
    slot: Mutex<Option<T>>,
    notify_any: Arc<Notify>,
}

impl<T> Latest1Queue<T> {
    pub fn new(notify_any: Arc<Notify>) -> Self {
        Self {
            slot: Mutex::new(None),
            notify_any,
        }
    }

    pub fn set(&self, value: T) {
        *self.slot.lock().expect("Latest1Queue poisoned") = Some(value);
        self.notify_any.notify_one();
    }

    pub fn try_recv(&self) -> Option<T> {
        self.slot.lock().expect("Latest1Queue poisoned").take()
    }
}
