use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;

use crate::events::PmuEvent;
use crate::network::master::MasterStation;

/// Per-connection event buffer. The frontend drains it via `poll_events`
/// rather than relying on `AppHandle::emit` + `listen()`, because the
/// Tauri 2 macOS WebKit `listen()` IPC deadlocks until the webview emits
/// a ready signal that Vue's mount alone doesn't always trigger — and
/// without buffering, every handshake event between `start_server` and
/// listener-attach is lost.
const MAX_BUFFER: usize = 4096;

#[derive(Default)]
pub struct EventBuffer {
    inner: StdMutex<VecDeque<PmuEvent>>,
}

impl EventBuffer {
    pub fn push(&self, ev: PmuEvent) {
        let mut q = self.inner.lock().expect("event buffer poisoned");
        if q.len() >= MAX_BUFFER {
            // Backpressure: drop the oldest. The UI polls every 100 ms so
            // overflow only happens when the frontend is paused (devtools
            // open, etc.). Dropping is better than blocking the network
            // task forwarding it.
            q.pop_front();
        }
        q.push_back(ev);
    }
    pub fn drain(&self) -> Vec<PmuEvent> {
        let mut q = self.inner.lock().expect("event buffer poisoned");
        q.drain(..).collect()
    }
}

pub struct AppState {
    pub master: Arc<Mutex<Option<MasterStation>>>,
    pub events: Arc<EventBuffer>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            master: Arc::new(Mutex::new(None)),
            events: Arc::new(EventBuffer::default()),
        }
    }
}
