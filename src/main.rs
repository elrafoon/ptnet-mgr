use std::{str::FromStr};

use serde::{Serialize, Deserialize};
use tokio::{time::{Duration, sleep}, net::TcpStream};
use log::{warn, info, error};

mod ptlink;
mod client_connection;

use client_connection::{ClientConnection};

#[derive(Debug,Serialize,Deserialize)]
pub struct Configuration {
    server_address: String,
    t_reconnect: Duration
}

async fn client_connect(conf: &Configuration) -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::net::SocketAddr::from_str(&conf.server_address)?;

    loop {
        info!("Connecting to {}", conf.server_address);

        let mut stream = match TcpStream::connect(addr).await {
            Err(err) => {
                error!("Error connecting to ptlink server at {}! {}", addr, err);
                tokio::time::sleep(conf.t_reconnect).await;
                continue;
            },
            Ok(stream) => {
                info!("Connected to ptlink server at {}", addr);
                stream
            }
        };

        // connected
        let mut cli = ClientConnection::new(&mut stream);
        match cli.dispatch().await {
            Err(err) => error!("Dispatcher terminated with error! ({err})"),
            Ok(_) => warn!("Dispatcher terminated without error")
        }

        sleep(conf.t_reconnect).await;
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut conf = Configuration {
        server_address: String::from("127.0.0.1:9885"),
        t_reconnect: Duration::from_secs(10)
    };

    client_connect(&conf).await?;

    Ok(())
}
