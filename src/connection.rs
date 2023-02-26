use std::io::Cursor;

use futures::{SinkExt};
use futures_util::StreamExt;
use log::{info, error};
use tokio::net::TcpStream;
use tungstenite::{Result, Error};
use uuid::Uuid;

use crate::{in_message::{read_message, WorldHostInMessage}, out_message::WorldHostOutMessage};

pub async fn accept_connection(stream: TcpStream) {
    if let Err(e) = handle_connection(stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(stream: TcpStream) -> Result<()> {
    let addr = stream.peer_addr()?;
    let connection_id = Uuid::new_v4();
    info!("Connection opened from {}. ID {}.", addr, connection_id);

    let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_binary() {
            let message = match read_message(Cursor::new(msg.into_data())).await {
                Ok(message) => message,
                Err(err) => {
                    ws_stream.send(WorldHostOutMessage::Error(err.to_string()).write().await?).await?;
                    return Err(Error::ConnectionClosed);
                }
            };
            match message {
                WorldHostInMessage::ListOnline { friends: _ } => {
                    // TODO: Implement
                },
                WorldHostInMessage::IsOnlineTo { user: _, is_online: _ } => {
                    // TODO: Implement
                }
            }
        }
    }

    Ok(())
}
