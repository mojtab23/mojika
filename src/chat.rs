use uuid::Uuid;

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
    pub id: String,
    /// peer_id
    pub sender: String,
    pub content: Content,
}

impl Message {
    pub fn new_text(sender: &str, text: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender: sender.to_string(),
            content: Content::Text { text },
        }
    }
    pub fn new_file(sender: &str, file_id: String, filename: String, progress: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender: sender.to_string(),
            content: Content::File {
                file_id,
                filename,
                progress,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum Content {
    Text {
        text: String,
    },
    File {
        file_id: String,
        filename: String,
        progress: String,
    },
}
