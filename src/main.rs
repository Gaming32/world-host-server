use argparse::{ArgumentParser, Store};
use connection::ConnectionsSet;
use log::{info, error};
use tokio::{net::TcpListener, sync::RwLock};

use std::{io::Error, sync::Arc};

use crate::connection::ConnectionsSetSync;

mod c2s_message;
mod connection;
mod s2c_message;
mod util;

#[derive(Debug)]
pub struct ServerConfig {
    addr: String,
    base_ip: String,
    java_port: u16
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut config: ServerConfig = ServerConfig {
        addr: "0.0.0.0:9646".to_string(),
        base_ip: "".to_string(),
        java_port: 25565
    };
    {
        let mut parser = ArgumentParser::new();
        parser.refer(&mut config.addr).add_option(&["-a", "--addr"], Store, "Address to run on");
        parser.refer(&mut config.base_ip).add_option(&["-b", "--base-addr"], Store, "Base address to use for proxy connections");
        parser.refer(&mut config.java_port).add_option(&["-J", "--java-port"], Store, "Port to use for Java Edition proxy connections");
        parser.parse_args_or_exit();
    }

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();
    info!("Starting world-host-server with {:?}", config);

    let listener = match TcpListener::bind(&config.addr).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind: {}", e);
            return Result::Err(e);
        }
    };
    info!("Listening on {}", config.addr);

    let connections: ConnectionsSet = Arc::new(RwLock::new(ConnectionsSetSync::new()));
    let config = Arc::new(config);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(connection::accept_connection(stream, connections.clone(), config.clone()));
    }

    Ok(())
}
