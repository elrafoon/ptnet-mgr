use std::{str::FromStr, fs};

use futures::future::{try_join_all};
use serde::{Serialize, Deserialize};
use tokio::{time::{Duration, sleep}, net::{TcpStream, tcp::WriteHalf}, sync::Mutex};
use log::{warn, info, error, debug};
use clap::{Parser};

mod ptnet;
mod client_connection;
mod database;
mod ptnet_process;
mod sol;

use client_connection::{ClientConnection};
use database::{Database};

use crate::{client_connection::{ClientConnectionDispatcher, ClientConnectionSender}, database::{node_address_to_string, node_table::NodeRecord}, ptnet_process::{NodeScanProcess, PersistProcess}};

#[derive(Parser,Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// configuration file
    config: Option<String>
}

#[derive(Debug,Serialize,Deserialize)]
pub enum NodeModelSource {
    /// don't load initial node seed, only detect nodes
    None,
    /// load initial node seed from SOL model
    SOL(String /* model root */),
}

#[derive(Debug,Serialize,Deserialize)]
pub struct Configuration {
    /// ptlink server address
    server_address: String,
    /// ptlink reconnect interval
    t_reconnect: u64,
    /// where to load initial node list from
    node_model_source: NodeModelSource
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            server_address: "127.0.0.1:9885".to_string(),
            t_reconnect: 10,
            node_model_source: NodeModelSource::SOL("/var/lib/kvds".to_string())
        }
    }
}

impl Configuration {
    fn reconnect_duration(&self) -> Duration {
        Duration::from_secs(self.t_reconnect)
    }
}

async fn client_connect<'a,'evt>(conf: &Configuration, db: &Database<'a>) -> Result<(), Box<dyn std::error::Error>>
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
            Box::new(NodeScanProcess::new(
                Duration::from_secs(10),
                db,
                &conn,
                &sender
            )),
            Box::new(PersistProcess::new(
                db,
                &conn
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

    info!("Loading ptnet-mgr database");
    let redb_db = redb::Database::create("ptnet-mgr.redb")?;
    let mut db = Database::new(&redb_db);
    db.init()?;
    // db.load()?;
    info!("Database loaded");

    match &conf.node_model_source {
        NodeModelSource::None => {},
        NodeModelSource::SOL(model_root) => {
            let model_nodes = sol::loader::load(model_root)?;
            let nodes = db.nodes.list()?;

            let new_nodes: Vec<&NodeRecord> = model_nodes.iter()
                .filter(|node| !nodes.contains(&node.address))
                .collect();

            info!("Add {} new nodes", new_nodes.len());
            db.nodes.update_many(new_nodes.iter().map(|node| *node), database::UpdateMode::MustCreate)?;

            let sz = db.nodes.len()?;

            db.nodes.remove_many(nodes
                .iter()
                .filter(|org_node| { !model_nodes.iter().any(|node| **org_node == node.address) })
            )?;

            info!("Remove {} non-existent nodes", sz - db.nodes.len()?);
        }
    };

    client_connect(
        &conf,
        &db
    ).await?;

    Ok(())
}
