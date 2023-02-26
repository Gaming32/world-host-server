use std::{io::Cursor, net::SocketAddr, borrow::Cow};

use futures::SinkExt;
use futures_util::StreamExt;
use log::{info, error, warn, debug};
use tokio::net::TcpStream;
use tungstenite::{Result, Error, protocol::{CloseFrame, frame::coding::CloseCode}};
use uuid::Uuid;

use crate::{in_message::{read_message, WorldHostInMessage}, out_message::WorldHostOutMessage};

#[derive(Debug)]
pub enum ConnectionState {
    Closed = 0,
    UPnP = 1,
    Proxy = 2
}

#[derive(Debug)]
pub struct Connection {
    id: Uuid,
    address: SocketAddr,
    username: String,
    state: ConnectionState
}

pub async fn accept_connection(stream: TcpStream) {
    if let Err(e) = handle_connection(stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(stream: TcpStream) -> Result<()> {
    let peer_addr = stream.peer_addr()?;
    debug!("Attempting connection from {}.", peer_addr);

    let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;

    let connection = if let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if !msg.is_text() {
            warn!("Invalid handshake from {}: not text ({:?}).", peer_addr, msg);
            return ws_stream.close(Some(CloseFrame {
                code: CloseCode::Invalid,
                reason: Cow::Borrowed("Invalid handshake: not text.")
            })).await;
        }
        Connection {
            id: Uuid::new_v4(),
            address: peer_addr,
            username: msg.into_text()?,
            state: ConnectionState::Closed
        }
    } else {
        warn!("Connection from {} terminated before handshake.", peer_addr);
        return Ok(());
    };
    info!("Connection opened: {:?}.", connection);

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_binary() {
            let message = match read_message(Cursor::new(msg.into_data())).await {
                Ok(message) => message,
                Err(err) => {
                    ws_stream.send(WorldHostOutMessage::Error(err.to_string()).write().await?).await?;
                    continue;
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

    info!("Connection closed: {:?}.", connection);

    Ok(())
}
