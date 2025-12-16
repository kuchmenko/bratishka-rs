use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use tokio::sync::Notify;

pub struct FifoDropOldestQueue<T> {
    inner: Arc<FifoDropOldestInner<T>>,
}

struct FifoDropOldestInner<T> {
    buf: Mutex<VecDeque<T>>,
    capacity: usize,
    notify_any: Arc<Notify>,
}

pub struct FifoDropOldestReceiver<T> {
    inner: Arc<FifoDropOldestInner<T>>,
}

impl<T> FifoDropOldestQueue<T> {
    pub fn new(capacity: usize, notify_any: Arc<Notify>) -> Self {
        assert!(capacity > 0);

        Self {
            inner: Arc::new(FifoDropOldestInner {
                buf: Mutex::new(VecDeque::with_capacity(capacity)),
                capacity,
                notify_any,
            }),
        }
    }

    pub fn push_overwrite(&self, value: T) {
        let mut buf = self.inner.buf.lock().expect("FifoDropOldestQueue poisoned");
        if buf.len() >= self.inner.capacity {
            let _ = buf.pop_front();
        }
        buf.push_back(value);
        drop(buf);
        self.inner.notify_any.notify_one();
    }

    pub fn receiver(&self) -> FifoDropOldestReceiver<T> {
        FifoDropOldestReceiver {
            inner: self.inner.clone(),
        }
    }
}

impl<T> FifoDropOldestReceiver<T> {
    pub fn try_recv(&self) -> Option<T> {
        self.inner
            .buf
            .lock()
            .expect("FifoDropOldestQueue poisoned")
            .pop_front()
    }
}
