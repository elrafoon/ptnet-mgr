use std::{time::Duration, sync::Arc, future::Future};
use async_trait::async_trait;

use futures::future::BoxFuture;
use log::{info, debug};
use tokio::{time::{interval}, task::{JoinHandle, self}};

use crate::{database::{Database, NodeRecord}};
use crate::ptnet::*;
use crate::ptnet::ptnet_c;
use crate::client_connection::{ClientConnection, Message, ClientConnectionSender};
use crate::ptnet_process::{PtNetProcess};

use crate::ptnet::ptnet_c::{BIT_PRM, FC_PRM_SEND_NOREPLY};

pub struct NodeScanProcess<'a> {
    scan_period: Duration,
    db: &'a Database<'a>,
    conn: &'a ClientConnection,
    sender: &'a ClientConnectionSender<'a>
}

#[async_trait]
impl<'a> PtNetProcess for NodeScanProcess<'a> {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut interval = interval(self.scan_period);
        loop {
            for node in self.db.nodes.values() {
                self.scan(node).await?;
                interval.tick().await;
                debug!("tick");
            }

            if self.db.nodes.is_empty() {
                interval.tick().await;
                debug!("tick");
            }
        }
    }
}

impl<'a> NodeScanProcess<'a> {
    pub fn new(scan_period: Duration, db: &'a Database, conn: &'a ClientConnection, sender: &'a ClientConnectionSender<'a>) -> Self {
        NodeScanProcess {
            scan_period: scan_period,
            db: db,
            conn: conn,
            sender: sender
        }
    }

    async fn scan(&mut self, node: &NodeRecord) -> Result<(), Box<dyn std::error::Error>> {
        info!("Scan node {}", node.mac());

        let msg = Message {
            port: PORT_AUTO,
            header: Header {
                C: (BIT_PRM | FC_PRM_SEND_NOREPLY) as u8,
                address: node.address,
            },
            payload: vec![10],
        };

        debug!("Transmit request");
        let rcvr = self.sender.send_message(&msg).await?;
        debug!("Await response");
        let result = rcvr.await?;
        debug!("result = {result}");
        Ok(())
    }
}