#[derive(Debug, Clone)]
pub struct Chat {
    pub messages: Vec<Message>,
}

impl Chat {
    pub fn new() -> Self {
        Self { messages: vec![] }
    }
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone)]
pub struct Message {
    /// peer_id
    pub sender: String,
    pub content: String,
}

impl Message {
    pub fn new(sender: &str, content: String) -> Self {
        Self {
            sender: sender.to_string(),
            content,
        }
    }
}
