use std::{io::Cursor, net::SocketAddr, borrow::Cow, sync::Arc, collections::HashMap, fmt::{Display, Debug}};

use futures::SinkExt;
use futures_util::StreamExt;
use log::{info, error, warn, debug};
use tokio::{net::TcpStream, sync::RwLock};
use tokio_tungstenite::WebSocketStream;
use tungstenite::{Result, Error, protocol::{CloseFrame, frame::coding::CloseCode}};
use uuid::Uuid;

use crate::{c2s_message::{WorldHostC2SMessage, read_uuid}, s2c_message::WorldHostS2CMessage, ServerConfig};

#[derive(Debug)]
pub enum JoinType {
    UPnP { port: u16 },
    Proxy
}

pub struct Connection {
    id: Uuid,
    address: SocketAddr,
    user_uuid: Uuid,
    stream: WebSocketStream<TcpStream>
}

impl Display for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Connection(id={}, addr={}, user={})", self.id, self.address, self.user_uuid)
    }
}

pub struct ConnectionsSetSync {
    connections: HashMap<Uuid, Arc<RwLock<Connection>>>,
    connections_by_user_id: HashMap<Uuid, Vec<Uuid>>
}

impl ConnectionsSetSync {
    pub fn new() -> ConnectionsSetSync {
        ConnectionsSetSync {
            connections: HashMap::new(),
            connections_by_user_id: HashMap::new()
        }
    }

    pub fn by_id(&self, id: &Uuid) -> Option<&Arc<RwLock<Connection>>> {
        self.connections.get(id)
    }

    pub fn by_user_id(&self, user_id: &Uuid) -> Option<&Vec<Uuid>> {
        self.connections_by_user_id.get(user_id)
    }

    pub async fn add(&mut self, connection: &Arc<RwLock<Connection>>) {
        let locked_connection = connection.read().await;
        self.connections_by_user_id.entry(locked_connection.user_uuid.clone())
            .or_insert_with(|| Vec::<Uuid>::new())
            .push(locked_connection.id);
        self.connections.insert(locked_connection.id, connection.clone());
    }
}

pub type ConnectionsSet = Arc<RwLock<ConnectionsSetSync>>;

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
        Arc::new(RwLock::new(Connection {
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
            stream: ws_stream
        }))
    } else {
        warn!("Connection from {} terminated before handshake.", peer_addr);
        return Ok(());
    };
    info!("Connection opened: {}.", *connection.read().await);
    let my_connection_id = connection.read().await.id.clone();

    connections.clone().write().await.add(&connection.clone()).await;

    loop {
        let mut connection = connection.write().await;
        let msg = match connection.stream.next().await {
            Some(msg) => msg?,
            None => break
        };
        if msg.is_binary() {
            let message = match WorldHostC2SMessage::read(Cursor::new(msg.into_data())).await {
                Ok(message) => message,
                Err(err) => {
                    connection
                        .stream
                        .send(WorldHostS2CMessage::Error {
                            message: err.to_string()
                        }.write().await?)
                        .await?;
                    continue;
                }
            };
            // if log_enabled!(Level::Debug) {
                info!("Received message {:?}", message);
            // }
            let lock = connections.clone();
            let connections_locked = lock.read().await;
            match message {
                WorldHostC2SMessage::ListOnline { friends } => {
                    let message = WorldHostS2CMessage::IsOnlineTo {
                        user: connection.user_uuid
                    }.write().await?;
                    for friend in friends {
                        if let Some(connection_ids) = connections_locked.by_user_id(&friend) {
                            for conn_id in connection_ids {
                                if *conn_id == my_connection_id {
                                    continue;
                                }
                                if let Some(conn) = connections_locked.by_id(conn_id) {
                                    // if conn.try_lock().is_err() {
                                    //     error!("DEADLOCK!!!");
                                    // }
                                    conn.write().await.stream.send(message.clone()).await?;
                                }
                            }
                        }
                    }
                }
                WorldHostC2SMessage::FriendRequest { to_user } => {
                    let message = WorldHostS2CMessage::FriendRequest {
                        from_user: connection.user_uuid
                    }.write().await?;
                    if let Some(connection_ids) = connections_locked.by_user_id(&to_user) {
                        for conn_id in connection_ids {
                            if *conn_id == my_connection_id {
                                continue;
                            }
                            if let Some(conn) = connections_locked.by_id(conn_id) {
                                conn.write().await.stream.send(message.clone()).await?;
                            }
                        }
                    }
                }
                WorldHostC2SMessage::PublishedWorld { friends } => {
                    let message = WorldHostS2CMessage::PublishedWorld {
                        user: connection.user_uuid
                    }.write().await?;
                    for friend in friends {
                        if let Some(connection_ids) = connections_locked.by_user_id(&friend) {
                            for conn_id in connection_ids {
                                if *conn_id == my_connection_id {
                                    continue;
                                }
                                if let Some(conn) = connections_locked.by_id(conn_id) {
                                    // if conn.try_lock().is_err() {
                                    //     error!("DEADLOCK!!!");
                                    // }
                                    conn.write().await.stream.send(message.clone()).await?;
                                }
                            }
                        }
                    }
                }
                WorldHostC2SMessage::ClosedWorld { friends } => {
                    let message = WorldHostS2CMessage::ClosedWorld {
                        user: connection.user_uuid
                    }.write().await?;
                    for friend in friends {
                        if let Some(connection_ids) = connections_locked.by_user_id(&friend) {
                            for conn_id in connection_ids {
                                if *conn_id == my_connection_id {
                                    continue;
                                }
                                if let Some(conn) = connections_locked.by_id(conn_id) {
                                    // if conn.try_lock().is_err() {
                                    //     error!("DEADLOCK!!!");
                                    // }
                                    conn.write().await.stream.send(message.clone()).await?;
                                }
                            }
                        }
                    }
                }
                WorldHostC2SMessage::RequestJoin { friend } => {
                    let message = WorldHostS2CMessage::RequestJoin {
                        user: friend,
                        connection_id: connection.id
                    }.write().await?;
                    if let Some(connection_ids) = connections_locked.by_user_id(&friend) {
                        if let Some(conn_id) = connection_ids.last() {
                            if *conn_id == my_connection_id {
                                continue;
                            }
                            if let Some(conn) = connections_locked.by_id(conn_id) {
                                conn.write().await.stream.send(message.clone()).await?;
                            }
                        }
                    }
                }
                WorldHostC2SMessage::JoinGranted { connection_id, join_type } => {
                    let message = match join_type {
                        JoinType::UPnP { port } => WorldHostS2CMessage::OnlineGame {
                            ip: peer_addr.to_string(),
                            port
                        },
                        JoinType::Proxy => WorldHostS2CMessage::OnlineGame {
                            ip: "connect0000-".to_string() + &connection.id.to_string() + "." + &config.base_ip,
                            port: config.java_port
                        }
                    }.write().await?;
                    if connection_id == my_connection_id {
                        continue;
                    }
                    if let Some(conn) = connections_locked.by_id(&connection_id) {
                        conn.write().await.stream.send(message.clone()).await?;
                    }
                }
                WorldHostC2SMessage::QueryRequest { friend } => {
                    let message = WorldHostS2CMessage::QueryRequest {
                        friend: connection.user_uuid,
                        connection_id: connection.id
                    }.write().await?;
                    if let Some(connection_ids) = connections_locked.by_user_id(&friend) {
                        if let Some(conn_id) = connection_ids.last() {
                            if *conn_id == my_connection_id {
                                continue;
                            }
                            if let Some(conn) = connections_locked.by_id(conn_id) {
                                conn.write().await.stream.send(message.clone()).await?;
                            }
                        }
                    }
                }
                WorldHostC2SMessage::QueryResponse { connection_id, data } => {
                    let message = WorldHostS2CMessage::QueryResponse {
                        friend: connection.user_uuid,
                        data
                    }.write().await?;
                    if connection_id == my_connection_id {
                        continue;
                    }
                    if let Some(conn) = connections_locked.by_id(&connection_id) {
                        conn.write().await.stream.send(message.clone()).await?;
                    }
                }
            }
            drop(connections_locked);
            drop(lock);
        }
    }

    info!("Connection closed: {}.", *connection.read().await);
    Ok(())
}
