use futures::SinkExt;
use futures_util::StreamExt;
use log::{info, error};
use tokio::net::TcpStream;
use tungstenite::{Result, Error};

use crate::{in_message::read_message, out_message::WorldHostOutMessage};

pub async fn accept_connection(stream: TcpStream) {
    if let Err(e) = handle_connection(stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => error!("Error processing connection: {}", err)
        }
    }
}

async fn handle_connection(stream: TcpStream) -> Result<()> {
    let addr = stream.peer_addr()?;
    info!("Connection opened from {}", addr);

    let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_binary() {
            let data = &msg.into_data() as &[u8];
            let _message = match read_message(data).await {
                Ok(message) => message,
                Err(err) => {
                    ws_stream.send(WorldHostOutMessage::Error(err.to_string()).write()).await?;
                    return Err(Error::ConnectionClosed);
                }
            };
        }
    }

    Ok(())
}
