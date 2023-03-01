use std::{error::Error, fmt::Display, io::{Cursor}};

use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::{util::DynResult, connection::JoinType};

#[derive(Debug)]
pub struct PresentMessageError(String);

impl Display for PresentMessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for PresentMessageError {
}

#[derive(Debug)]
pub enum WorldHostC2SMessage {
    ListOnline { friends: Vec<Uuid> },
    IsOnlineTo { connection_id: Uuid },
    FriendRequest { to_user: Uuid },
    PublishedWorld { friends: Vec<Uuid> },
    ClosedWorld { friends: Vec<Uuid> },
    RequestJoin { friend: Uuid },
    JoinGranted { connection_id: Uuid, join_type: JoinType },
}

impl WorldHostC2SMessage {
    pub async fn read(mut reader: Cursor<Vec<u8>>) -> DynResult<WorldHostC2SMessage> {
        match reader.read_u8().await? {
            0 => {
                let count = reader.read_u32().await?;
                let mut result = Vec::new();
                for _ in 0..count {
                    result.push(read_uuid(&mut reader).await?);
                }
                Ok(Self::ListOnline { friends: result })
            }
            1 => Ok(Self::IsOnlineTo {
                connection_id: read_uuid(&mut reader).await?
            }),
            2 => Ok(Self::FriendRequest {
                to_user: read_uuid(&mut reader).await?
            }),
            3 => {
                let count = reader.read_u32().await?;
                let mut result = Vec::new();
                for _ in 0..count {
                    result.push(read_uuid(&mut reader).await?);
                }
                Ok(Self::PublishedWorld { friends: result })
            }
            4 => {
                let count = reader.read_u32().await?;
                let mut result = Vec::new();
                for _ in 0..count {
                    result.push(read_uuid(&mut reader).await?);
                }
                Ok(Self::ClosedWorld { friends: result })
            }
            5 => Ok(Self::RequestJoin {
                friend: read_uuid(&mut reader).await?
            }),
            6 => Ok(Self::JoinGranted {
                connection_id: read_uuid(&mut reader).await?,
                join_type: match reader.read_u8().await? {
                    0 => JoinType::UPnP { port: reader.read_u16().await? },
                    1 => JoinType::Proxy,
                    join_type_id => return Err(Box::new(PresentMessageError(
                        format!("Received packet with unknown join_type_id from client: {join_type_id}")
                    )))
                }
            }),
            type_id => Err(Box::new(PresentMessageError(
                format!("Received packet with unknown type_id from client: {type_id}")
            )))
        }
    }
}

pub async fn read_uuid(reader: &mut Cursor<Vec<u8>>) -> DynResult<Uuid> {
    Ok(Uuid::from_u64_pair(reader.read_u64().await?, reader.read_u64().await?))
}

async fn _read_string(reader: &mut Cursor<Vec<u8>>) -> DynResult<String> {
    let mut buf = Vec::with_capacity(reader.read_u16().await? as usize);
    std::io::Read::read_exact(reader, &mut buf)?;
    Ok(String::from_utf8(buf)?)
}
