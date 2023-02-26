use std::borrow::Cow;

use tokio::io::AsyncWriteExt;
use tungstenite::{Message, protocol::{CloseFrame, frame::coding::CloseCode}, Result};
use uuid::Uuid;

pub enum WorldHostOutMessage {
    Error(String),
    IsOnlineTo(String),
    OnlineGame(Uuid)
}

impl WorldHostOutMessage {
    pub async fn write(&self) -> Result<Message> {
        match self {
            Self::Error(message) => Ok(Message::Close(Option::Some(CloseFrame {
                code: CloseCode::Protocol,
                reason: Cow::Owned(message.to_owned())
            }))),
            Self::IsOnlineTo(user) => Ok(Message::Text(user.to_string())),
            Self::OnlineGame(connection_id) => {
                let mut vec = Vec::with_capacity(16);
                let (most, least) = connection_id.as_u64_pair();
                vec.write_u64(most).await?;
                vec.write_u64(least).await?;
                Ok(Message::Binary(vec))
            }
        }
    }
}
