use std::{io::Cursor};

use tokio::io::AsyncWriteExt;
use tungstenite::{Message, Result};
use uuid::Uuid;

pub enum WorldHostOutMessage {
    Error { message: String },
    IsOnlineTo { user: String, connection_id: Uuid },
    OnlineGame { ip: String }
}

impl WorldHostOutMessage {
    pub async fn write(&self) -> Result<Message> {
        let mut vec = Vec::with_capacity(16);
        let mut writer = Cursor::new(&mut vec);
        match self {
            Self::Error { message } => {
                writer.write_u8(0).await?;
                write_string(&mut writer, message).await?;
            },
            Self::IsOnlineTo { user, connection_id } => {
                writer.write_u8(1).await?;
                write_string(&mut writer, user).await?;
                let (most, least) = connection_id.as_u64_pair();
                writer.write_u64(most).await?;
                writer.write_u64(least).await?;
            },
            Self::OnlineGame { ip } => {
                writer.write_u8(2).await?;
                write_string(&mut writer, ip).await?;
            }
        };
        Ok(Message::Binary(vec))
    }
}

async fn write_string(writer: &mut Cursor<&mut Vec<u8>>, string: &str) -> Result<()> {
    writer.write(string.as_bytes()).await?;
    Ok(())
}
