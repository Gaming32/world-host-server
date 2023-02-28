use std::{io::Cursor};

use tokio::io::AsyncWriteExt;
use tungstenite::{Message, Result};
use uuid::Uuid;

pub enum WorldHostOutMessage {
    Error { message: String },
    IsOnlineTo { user: Uuid, connection_id: Uuid },
    OnlineGame { ip: String },
    FriendRequest { from_user: String },
    WentInGame { user: Uuid }
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
                write_uuid(&mut writer, user).await?;
                write_uuid(&mut writer, connection_id).await?;
            },
            Self::OnlineGame { ip } => {
                writer.write_u8(2).await?;
                write_string(&mut writer, ip).await?;
            },
            Self::FriendRequest { from_user } => {
                writer.write_u8(3).await?;
                write_string(&mut writer, from_user).await?;
            },
            Self::WentInGame { user } => {
                writer.write_u8(4).await?;
                write_uuid(&mut writer, user).await?;
            }
        };
        Ok(Message::Binary(vec))
    }
}

pub async fn write_uuid(writer: &mut Cursor<&mut Vec<u8>>, uuid: &Uuid) -> Result<()> {
    let (most, least) = uuid.as_u64_pair();
    writer.write_u64(most).await?;
    writer.write_u64(least).await?;
    Ok(())
}

async fn write_string(writer: &mut Cursor<&mut Vec<u8>>, string: &str) -> Result<()> {
    writer.write(string.as_bytes()).await?;
    Ok(())
}
