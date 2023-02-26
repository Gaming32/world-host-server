use std::{error::Error, fmt::Display};

use tokio::io::AsyncReadExt;

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
}

pub async fn read_message(mut data: &[u8]) -> Result<WorldHostInMessage, Box<dyn Error + Send + Sync>> {
    let type_id = data.read_u8().await?;

    Err(Box::new(UnknownTypeIdError(type_id)))
}
