use std::{error::Error, fmt::Display, io::{Cursor}};

use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::util::DynResult;

#[derive(Debug)]
pub struct UnknownTypeIdError(u8);

impl Display for UnknownTypeIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnknownTypeIdError({})", self.0)
    }
}

impl Error for UnknownTypeIdError {
}

pub enum WorldHostInMessage {
    ListOnline { friends: Vec<String> },
    IsOnlineTo { connection_id: Uuid }
}

pub async fn read_message(mut reader: Cursor<Vec<u8>>) -> DynResult<WorldHostInMessage> {
    match reader.read_u8().await? {
        0 => {
            let count = reader.read_u32().await?;
            let mut result = Vec::new();
            for _ in 0..count {
                result.push(read_string(&mut reader).await?);
            }
            Ok(WorldHostInMessage::ListOnline { friends: result })
        }
        1 => {
            Ok(WorldHostInMessage::IsOnlineTo {
                connection_id: Uuid::from_u64_pair(
                    reader.read_u64().await?,
                    reader.read_u64().await?
                )
            })
        }
        type_id => Err(Box::new(UnknownTypeIdError(type_id)))
    }
}

async fn read_string(reader: &mut Cursor<Vec<u8>>) -> DynResult<String> {
    let mut buf = Vec::with_capacity(reader.read_u16().await? as usize);
    std::io::Read::read_exact(reader, &mut buf)?;
    Ok(String::from_utf8(buf)?)
}
