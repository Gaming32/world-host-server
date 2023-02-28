use std::{io::Cursor, net::SocketAddr, borrow::Cow, sync::Arc, collections::HashMap, fmt::Display};

use futures::{SinkExt, lock::Mutex};
use futures_util::StreamExt;
use log::{info, error, warn, debug};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tungstenite::{Result, Error, protocol::{CloseFrame, frame::coding::CloseCode}};
use uuid::Uuid;

use crate::{in_message::{read_message, WorldHostInMessage, read_uuid}, out_message::WorldHostOutMessage, ServerConfig};

#[derive(Debug)]
#[repr(u8)]
pub enum ConnectionState {
    Closed = 0,
    UPnP { port: u16 } = 1,
    Proxy = 2
}

pub struct Connection {
    id: Uuid,
    address: SocketAddr,
    user_uuid: Uuid,
    state: ConnectionState,
    stream: WebSocketStream<TcpStream>
}

impl Display for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Connection(id={}, addr={}, user={})", self.id, self.address, self.user_uuid)
    }
}

pub struct ConnectionsSetSync {
    connections: HashMap<Uuid, Arc<Mutex<Connection>>>,
    connections_by_user_id: HashMap<Uuid, Vec<Uuid>>
}

impl ConnectionsSetSync {
    pub fn new() -> ConnectionsSetSync {
        ConnectionsSetSync {
            connections: HashMap::new(),
            connections_by_user_id: HashMap::new()
        }
    }

    pub fn by_id(&self, id: &Uuid) -> Option<&Arc<Mutex<Connection>>> {
        self.connections.get(id)
    }

    pub fn by_user_id(&self, user_id: &Uuid) -> Option<&Vec<Uuid>> {
        self.connections_by_user_id.get(user_id)
    }

    pub async fn add(&mut self, connection: &Arc<Mutex<Connection>>) {
        self.connections_by_user_id.entry(connection.lock().await.user_uuid.clone())
            .or_insert_with(|| Vec::<Uuid>::new())
            .push(connection.lock().await.id);
        self.connections.insert(connection.lock().await.id, connection.clone());
    }
}

pub type ConnectionsSet = Arc<Mutex<ConnectionsSetSync>>;

pub async fn accept_connection(stream: TcpStream, connections: ConnectionsSet, config: Arc<ServerConfig>) {
    if let Err(e) = handle_connection(stream, connections, config).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(stream: TcpStream, connections: ConnectionsSet, config: Arc<ServerConfig>) -> Result<()> {
    let peer_addr = stream.peer_addr()?;
    debug!("Attempting connection from {}.", peer_addr);

    let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;

    let connection = if let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if !msg.is_binary() {
            warn!("Invalid handshake from {}: not binary ({:?}).", peer_addr, msg);
            return ws_stream.close(Some(CloseFrame {
                code: CloseCode::Invalid,
                reason: Cow::Borrowed("Invalid handshake: not binary.")
            })).await;
        }
        Arc::new(Mutex::new(Connection {
            id: Uuid::new_v4(),
            address: peer_addr,
            user_uuid: match read_uuid(&mut Cursor::new(msg.into_data())).await {
                Ok(uuid) => uuid,
                Err(err) => {
                    warn!("Invalid handshake from {}: failed to read UUID ({}).", peer_addr, err);
                    return ws_stream.close(Some(CloseFrame {
                        code: CloseCode::Invalid,
                        reason: Cow::Borrowed("Invalid handshake: failed to read UUID.")
                    })).await;
                }
            },
            state: ConnectionState::Closed,
            stream: ws_stream
        }))
    } else {
        warn!("Connection from {} terminated before handshake.", peer_addr);
        return Ok(());
    };
    info!("Connection opened: {}.", *connection.lock().await);

    connections.lock().await.add(&connection).await;

    while let Some(msg) = connection.lock().await.stream.next().await {
        let msg = msg?;
        if msg.is_binary() {
            let message = match read_message(Cursor::new(msg.into_data())).await {
                Ok(message) => message,
                Err(err) => {
                    connection.lock()
                        .await
                        .stream
                        .send(WorldHostOutMessage::Error {
                            message: err.to_string()
                        }.write().await?)
                        .await?;
                    continue;
                }
            };
            match message {
                WorldHostInMessage::ListOnline { friends } => {
                    let connection = connection.lock().await;
                    let message = WorldHostOutMessage::IsOnlineTo {
                        user: connection.user_uuid,
                        connection_id: connection.id
                    }.write().await?;
                    for friend in friends {
                        let connections = connections.lock().await;
                        if let Some(connection_ids) = connections.by_user_id(&friend) {
                            for conn_id in connection_ids {
                                if let Some(conn) = connections.by_id(conn_id) {
                                    conn.lock().await.stream.send(message.clone()).await?;
                                }
                            }
                        }
                    }
                },
                WorldHostInMessage::IsOnlineTo { connection_id } => {
                    let mut connection = connection.lock().await;
                    let message = WorldHostOutMessage::OnlineGame { ip: match connection.state {
                        ConnectionState::Closed => continue,
                        ConnectionState::UPnP { port } => connection.address.to_string() + ":" + &port.to_string(),
                        ConnectionState::Proxy if !config.base_ip.is_empty() =>
                            "connect0000-".to_string() + &connection.id.to_string() + "." + &config.base_ip,
                        ConnectionState::Proxy => {
                            connection.stream
                                .send(WorldHostOutMessage::Error {
                                    message: "This World Host server does not support Proxy mode hosting.".to_string()
                                }.write().await?)
                                .await?;
                            continue;
                        }
                    }}.write().await?;
                    if let Some(conn) = connections.lock().await.by_id(&connection_id) {
                        conn.lock().await.stream.send(message.clone()).await?;
                    }
                }
                WorldHostInMessage::FriendRequest { to_user } => {
                    let message = WorldHostOutMessage::FriendRequest {
                        from_user: connection.lock().await.user_uuid.to_string()
                    }.write().await?;
                    let connections = connections.lock().await;
                    if let Some(connection_ids) = connections.by_user_id(&to_user) {
                        for conn_id in connection_ids {
                            if let Some(conn) = connections.by_id(conn_id) {
                                conn.lock().await.stream.send(message.clone()).await?;
                            }
                        }
                    }
                },
                WorldHostInMessage::WentInGame { friends } => {
                    let connection = connection.lock().await;
                    let message = WorldHostOutMessage::WentInGame {
                        user: connection.user_uuid
                    }.write().await?;
                    for friend in friends {
                        let connections = connections.lock().await;
                        if let Some(connection_ids) = connections.by_user_id(&friend) {
                            for conn_id in connection_ids {
                                if let Some(conn) = connections.by_id(conn_id) {
                                    conn.lock().await.stream.send(message.clone()).await?;
                                }
                            }
                        }
                    }
                },
            }
        }
    }

    info!("Connection closed: {}.", *connection.lock().await);
    Ok(())
}
