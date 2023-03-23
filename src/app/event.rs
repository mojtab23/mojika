use tokio::sync::broadcast::{Receiver, Sender};

#[derive(Debug)]
pub struct AppEvent {
    pub send_text_signal: Sender<String>,
    _send_text_signal_r: Receiver<String>,
}

impl AppEvent {
    pub fn new() -> Self {
        let (one_s, one_r) = tokio::sync::broadcast::channel(10);
        Self {
            send_text_signal: one_s,
            _send_text_signal_r: one_r,
        }
    }
}

impl Default for AppEvent {
    fn default() -> Self {
        Self::new()
    }
}
