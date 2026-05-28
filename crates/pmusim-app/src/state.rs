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
            // Backpressure: prefer to drop the oldest DataFrame/RawFrame
            // rather than the oldest event of any type. Lifecycle events
            // (SessionCreated, Cfg1Received, StreamingStarted, ...) drive
            // UI state transitions; losing one strands the rest. Frame
            // data is a high-frequency stream where one missing sample is
            // invisible. Fallback to FIFO if everything is lifecycle.
            let drop_idx = q.iter().position(|e| {
                matches!(e, PmuEvent::DataFrame { .. } | PmuEvent::RawFrame { .. })
            });
            if let Some(i) = drop_idx {
                q.remove(i);
            } else {
                q.pop_front();
            }
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
