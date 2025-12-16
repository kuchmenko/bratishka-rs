use std::{pin::Pin, sync::Arc};

use tokio::sync::{Notify, mpsc};

pub struct IsolatedForwarder<T> {
    inbox_tx: mpsc::Sender<T>,
}

pub struct StartupTasks {
    pub tokio: Vec<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl<T: Send + 'static> IsolatedForwarder<T> {
    pub fn new(
        output_buffer: usize,
        notify_any: Arc<Notify>,
    ) -> (
        IsolatedForwarder<T>,
        mpsc::Receiver<T>,
        Pin<Box<dyn Future<Output = ()> + Send>>,
    ) {
        let (inbox_tx, mut inbox_rx) = mpsc::channel::<T>(16);
        let (out_tx, out_rx) = mpsc::channel::<T>(output_buffer);

        let drain_task = Box::pin(async move {
            while let Some(value) = inbox_rx.recv().await {
                if out_tx.send(value).await.is_err() {
                    break;
                }
                notify_any.notify_one();
            }
        });

        (IsolatedForwarder { inbox_tx }, out_rx, drain_task)
    }

    pub fn try_send(&self, value: T) -> Result<(), T> {
        self.inbox_tx.try_send(value).map_err(|e| e.into_inner())
    }
}
