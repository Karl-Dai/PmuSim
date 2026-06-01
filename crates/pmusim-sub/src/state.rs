use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;

use crate::events::SubEvent;
use crate::network::substation::SubStation;

const MAX_BUFFER: usize = 4096;

#[derive(Default)]
pub struct EventBuffer {
    inner: StdMutex<VecDeque<SubEvent>>,
}

impl EventBuffer {
    pub fn push(&self, ev: SubEvent) {
        let mut q = self.inner.lock().expect("event buffer poisoned");
        if q.len() >= MAX_BUFFER {
            // 优先丢高频帧事件，保留生命周期事件。
            let drop_idx = q.iter().position(|e| {
                matches!(e, SubEvent::DataFrameSent { .. } | SubEvent::RawFrame { .. })
            });
            if let Some(i) = drop_idx { q.remove(i); } else { q.pop_front(); }
        }
        q.push_back(ev);
    }
    pub fn drain(&self) -> Vec<SubEvent> {
        let mut q = self.inner.lock().expect("event buffer poisoned");
        q.drain(..).collect()
    }
}

pub struct AppState {
    pub sub: Arc<Mutex<Option<SubStation>>>,
    pub events: Arc<EventBuffer>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sub: Arc::new(Mutex::new(None)),
            events: Arc::new(EventBuffer::default()),
        }
    }
}
