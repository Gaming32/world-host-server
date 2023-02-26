use argparse::{ArgumentParser, Store};
use log::{info, error};
use tokio::net::TcpListener;
use std::io::Error;

mod connection;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut addr = "0.0.0.0:9646".to_string();
    {
        let mut parser = ArgumentParser::new();
        parser.refer(&mut addr).add_option(&["-a", "--addr"], Store, "Address to run on");
        parser.parse_args_or_exit();
    }

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();
    info!("Starting world-host-server...");

    let listener = match TcpListener::bind(&addr).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind: {}", e);
            return Result::Err(e);
        }
    };
    info!("Listening on {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(connection::process_connection(stream));
    }

    Ok(())
}
