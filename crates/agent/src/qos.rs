use std::collections::VecDeque;
use thiserror::Error;

/// Interactive/control traffic precedes media; bulk file chunks yield to both.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum TrafficClass {
    Input = 0,
    WindowVideo = 1,
    DisplayVideo = 2,
    Clipboard = 3,
    SmallFile = 4,
    BulkFile = 5,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum EnqueueError {
    #[error("bounded queue is full; producer must apply backpressure")]
    Full,
    #[error("item exceeds the configured per-class byte budget")]
    ItemTooLarge,
}

/// Bounded per-class queues with deterministic priority selection and byte budgets.
/// When a video queue fills, old frames are discarded to avoid stale-frame lag.
pub struct QosQueue<T> {
    capacity_per_class: usize,
    max_bytes_per_class: usize,
    queued_bytes: [usize; 6],
    queues: [VecDeque<Queued<T>>; 6],
}

struct Queued<T> {
    item: T,
    bytes: usize,
}

impl<T> QosQueue<T> {
    pub fn new(capacity_per_class: usize, max_bytes_per_class: usize) -> Self {
        assert!(
            capacity_per_class > 0,
            "queue item capacity must be nonzero"
        );
        assert!(
            max_bytes_per_class > 0,
            "queue byte capacity must be nonzero"
        );
        Self {
            capacity_per_class,
            max_bytes_per_class,
            queued_bytes: [0; 6],
            queues: std::array::from_fn(|_| VecDeque::new()),
        }
    }

    pub fn enqueue(
        &mut self,
        class: TrafficClass,
        item: T,
        estimated_bytes: usize,
    ) -> Result<(), EnqueueError> {
        if estimated_bytes > self.max_bytes_per_class {
            return Err(EnqueueError::ItemTooLarge);
        }
        let index = class as usize;
        let queue = &mut self.queues[index];
        while queue.len() >= self.capacity_per_class
            || self.queued_bytes[index] > self.max_bytes_per_class - estimated_bytes
        {
            match class {
                TrafficClass::WindowVideo | TrafficClass::DisplayVideo if !queue.is_empty() => {
                    if let Some(discarded) = queue.pop_front() {
                        self.queued_bytes[index] -= discarded.bytes;
                    }
                }
                _ => return Err(EnqueueError::Full),
            }
        }
        queue.push_back(Queued {
            item,
            bytes: estimated_bytes,
        });
        self.queued_bytes[index] += estimated_bytes;
        Ok(())
    }

    pub fn dequeue(&mut self) -> Option<(TrafficClass, T)> {
        const ORDER: [TrafficClass; 6] = [
            TrafficClass::Input,
            TrafficClass::WindowVideo,
            TrafficClass::DisplayVideo,
            TrafficClass::Clipboard,
            TrafficClass::SmallFile,
            TrafficClass::BulkFile,
        ];
        ORDER.into_iter().find_map(|class| {
            let index = class as usize;
            self.queues[index].pop_front().map(|queued| {
                self.queued_bytes[index] -= queued.bytes;
                (class, queued.item)
            })
        })
    }

    pub fn len(&self) -> usize {
        self.queues.iter().map(VecDeque::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.queues.iter().all(VecDeque::is_empty)
    }

    pub fn queued_bytes(&self, class: TrafficClass) -> usize {
        self.queued_bytes[class as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_is_dequeued_before_display_and_bulk_file_work() {
        let mut queue = QosQueue::new(4, 1024);
        queue.enqueue(TrafficClass::BulkFile, "chunk", 10).unwrap();
        queue
            .enqueue(TrafficClass::DisplayVideo, "frame", 20)
            .unwrap();
        queue.enqueue(TrafficClass::Input, "pointer", 3).unwrap();
        assert_eq!(queue.dequeue(), Some((TrafficClass::Input, "pointer")));
        assert_eq!(queue.dequeue(), Some((TrafficClass::DisplayVideo, "frame")));
        assert_eq!(queue.dequeue(), Some((TrafficClass::BulkFile, "chunk")));
    }

    #[test]
    fn stale_video_is_dropped_to_keep_item_and_byte_limits() {
        let mut queue = QosQueue::new(4, 25);
        queue.enqueue(TrafficClass::WindowVideo, 1, 10).unwrap();
        queue.enqueue(TrafficClass::WindowVideo, 2, 10).unwrap();
        queue.enqueue(TrafficClass::WindowVideo, 3, 10).unwrap();
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.queued_bytes(TrafficClass::WindowVideo), 20);
        assert_eq!(queue.dequeue(), Some((TrafficClass::WindowVideo, 2)));
        assert_eq!(queue.dequeue(), Some((TrafficClass::WindowVideo, 3)));
        assert_eq!(queue.queued_bytes(TrafficClass::WindowVideo), 0);
    }

    #[test]
    fn full_bulk_queue_backpressures_instead_of_allocating_unbounded_memory() {
        let mut queue = QosQueue::new(1, 10);
        queue.enqueue(TrafficClass::BulkFile, "one", 5).unwrap();
        assert_eq!(
            queue.enqueue(TrafficClass::BulkFile, "two", 1),
            Err(EnqueueError::Full)
        );
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn queue_rejects_a_single_oversized_frame() {
        let mut queue = QosQueue::new(4, 10);
        assert_eq!(
            queue.enqueue(TrafficClass::DisplayVideo, "large", 11),
            Err(EnqueueError::ItemTooLarge)
        );
        assert!(queue.is_empty());
    }
}
