use std::{error::Error, fmt::Display, io::{Cursor}};

use tokio::io::AsyncReadExt;

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
    IsOnlineTo { user: String, is_online: bool }
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
                user: read_string(&mut reader).await?,
                is_online: reader.read_u8().await? != 0
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
