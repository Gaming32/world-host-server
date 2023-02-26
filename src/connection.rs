use log::{info, error};
use tokio::net::TcpStream;

pub async fn process_connection(stream: TcpStream) {
    let addr = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Failed to get remote address: {}", e);
            return;
        }
    };
    info!("Connection opened from {}", addr);
}
