pub mod fifo_drop_oldest_queue;
pub mod isolated_forwarder;
pub mod latest1_queue;

pub use fifo_drop_oldest_queue::*;
pub use isolated_forwarder::*;
pub use latest1_queue::*;

pub enum QueueKind {
    Latest1,
    FifoDropOldest { capacity: usize },
    BoundedDropNewest { capacity: usize },
    Isolated { output_buffer: usize },
}
