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
    pub content: Content,
}

impl Message {
    pub fn new_text(sender: &str, text: String) -> Self {
        Self {
            sender: sender.to_string(),
            content: Content::Text { text },
        }
    }
    pub fn new_file(sender: &str, filename: String) -> Self {
        Self {
            sender: sender.to_string(),
            content: Content::File { filename },
        }
    }
}

#[derive(Debug, Clone)]
pub enum Content {
    Text { text: String },
    File { filename: String },
}
