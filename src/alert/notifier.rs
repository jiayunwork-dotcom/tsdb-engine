use std::sync::Arc;
use tokio::sync::broadcast;
use crate::alert::event::AlertEvent;

const MEMORY_QUEUE_CAPACITY: usize = 1000;

pub struct AlertNotifier {
    tx: broadcast::Sender<String>,
    queue: Arc<parking_lot::Mutex<Vec<AlertEvent>>>,
}

impl AlertNotifier {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            queue: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(MEMORY_QUEUE_CAPACITY))),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub fn notify(&self, event: &AlertEvent) {
        if let Ok(json) = serde_json::to_string(event) {
            let _ = self.tx.send(json);
        }

        let mut queue = self.queue.lock();
        queue.push(event.clone());
        if queue.len() > MEMORY_QUEUE_CAPACITY {
            let excess = queue.len() - MEMORY_QUEUE_CAPACITY;
            queue.drain(0..excess);
        }
    }

    pub fn recent_events(&self, limit: usize) -> Vec<AlertEvent> {
        let queue = self.queue.lock();
        let start = if queue.len() > limit {
            queue.len() - limit
        } else {
            0
        };
        queue[start..].to_vec()
    }
}
