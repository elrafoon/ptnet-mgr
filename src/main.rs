use std::{str::FromStr, fs, path::PathBuf};

use futures::future::{try_join_all};
use serde::{Serialize, Deserialize};
use tokio::{time::{Duration, sleep}, net::{TcpStream, tcp::WriteHalf}, sync::Mutex};
use log::{warn, info, error};
use clap::{Parser};

mod ptnet;
mod client_connection;
mod database;
mod schema;
mod ptnet_process;
mod nodescan_process;
mod helpers;

use client_connection::{ClientConnection};
use database::{Database};

use crate::{client_connection::{ClientConnectionDispatcher, ClientConnectionSender}, database::NodeRecord};

/// SOL background processing daemon
#[derive(Parser,Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// configuration file
    config: Option<String>
}

/// SOL background processing daemon
#[derive(Debug,Serialize,Deserialize)]
pub struct Configuration {
    /// ptlink server address
    server_address: String,
    /// ptlink reconnect interval
    t_reconnect: u64,
    /// sol model root dir
    sol_model_root: String
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            server_address: "127.0.0.1:9885".to_string(),
            t_reconnect: 10,
            sol_model_root: "/var/lib/kvds".to_string()
        }
    }
}

impl Configuration {
    fn reconnect_duration(&self) -> Duration {
        Duration::from_secs(self.t_reconnect)
    }
}

async fn client_connect<'a>(conf: &Configuration, _sol_user: &schema::UserModel, db: &Database<'a>) -> Result<(), Box<dyn std::error::Error>>
{
    let addr = std::net::SocketAddr::from_str(&conf.server_address)?;
    let t_reconnect = conf.reconnect_duration();

    loop {
        info!("Connecting to {}", conf.server_address);

        let mut stream = match TcpStream::connect(addr).await {
            Err(err) => {
                error!("Error connecting to ptlink server at {}! {}", addr, err);
                tokio::time::sleep(t_reconnect).await;
                continue;
            },
            Ok(stream) => {
                info!("Connected to ptlink server at {}", addr);
                stream
            }
        };

        let (mut reader, writer) = stream.split();
        let guarded_writer: Mutex<WriteHalf> = Mutex::new(writer);

        // connected
        let conn = ClientConnection::new();
        let sender = ClientConnectionSender::new(&conn, &guarded_writer);
        let mut dispatcher = ClientConnectionDispatcher::new(&conn, &mut reader);

        info!("Init connection");
        let mut processes: Vec<Box<dyn ptnet_process::PtNetProcess>> = vec![
            Box::new(nodescan_process::NodeScanProcess::new(
                Duration::from_secs(10),
                db,
                &conn,
                &sender
            ))
        ];

        //let dispatch = async || { dispatcher.dispatch() };
        let mut futures =
            Vec::from_iter(processes.iter_mut().map(|proc| proc.run()));

        futures.insert(0, Box::pin(dispatcher.dispatch()));

        let results = try_join_all(futures).await;

        match results {
            Err(err) => error!("Connection terminated with error! ({err})"),
            Ok(_) => warn!("Dispatcher terminated without error")
        }

        info!("Fini connection");

        sleep(t_reconnect).await;
    };
}



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let mut conf: Configuration = Default::default();
    let args = Args::parse();

    if let Some(conf_file) = args.config {
        conf = serde_json::from_reader(fs::File::open(conf_file)?)?;
    }

    let mut sol_user_path = PathBuf::from(&conf.sol_model_root);
    sol_user_path.push("sol.user.json");
    info!("Loading SOL user model from {}", sol_user_path.as_os_str().to_str().unwrap());
    let soluser: schema::UserModel = serde_json::from_reader(fs::File::open(sol_user_path)?)?;
    info!("Model loaded");

    info!("Loading sol-mgr database");
    let redb_db = redb::Database::create("sol-mgr.redb")?;
    let mut db = Database::new(&redb_db);
    db.init()?;
    db.load()?;
    info!("Database loaded");

    if let Some(network) = soluser.network.as_ref() {
        let nodes = &db.nodes;
        let mut new_nodes: Vec<NodeRecord> =
            network.ballasts.iter()
                .map(|ballast| helpers::parse_user_address(ballast.address.as_str()).unwrap())
                .filter(|address| !nodes.contains_key(address))
                .map(|address| NodeRecord { address: address, ..Default::default() })
                .collect();

        new_nodes.extend(
            network.sensors.iter()
                .filter(|e| e.part_of.is_none())
                .map(|sensor| helpers::parse_user_address(sensor.address.as_str()).unwrap())
                .filter(|address| !nodes.contains_key(address))
                .map(|address| NodeRecord { address: address, ..Default::default() })
        );

        info!("Add {} new nodes", new_nodes.len());
        db.add_nodes(new_nodes.iter())?;

        let sz = db.nodes.len();
        db.nodes.retain(|k, _e| {
            network.ballasts.iter().any(|b| *k == helpers::parse_user_address(b.address.as_str()).unwrap()) ||
            network.sensors.iter().any(|s| *k == helpers::parse_user_address(s.address.as_str()).unwrap())
        });
        info!("Remove {} non-existent nodes", sz - db.nodes.len());
    }

    client_connect(
        &conf,
        &soluser,
        &db
    ).await?;

    Ok(())
}
