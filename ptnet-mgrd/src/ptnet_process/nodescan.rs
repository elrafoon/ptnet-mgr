use std::{time::Duration};
use async_trait::async_trait;

use log::{info, debug, warn};
use tokio::{time::{interval, sleep}, sync::broadcast, select};

use crate::{database::{Database, NodeRecord}, client_connection::IOBMessage};
use crate::ptnet::*;
use crate::ptnet::ptnet_c;
use crate::client_connection::{ClientConnection, Message, ClientConnectionSender};
use crate::ptnet_process::{PtNetProcess};

use crate::ptnet::ptnet_c::{BIT_PRM, FC_PRM_SEND_NOREPLY};

pub struct NodeScanProcess<'a> {
    scan_period: Duration,
    db: &'a Database<'a>,
    conn: &'a ClientConnection,
    sender: &'a ClientConnectionSender<'a>,
    message_rcvr: broadcast::Receiver<IOBMessage>
}

#[async_trait]
impl<'a> PtNetProcess for NodeScanProcess<'a> {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut interval = interval(self.scan_period);
        loop {
            let node_records = self.db.load_nodes(self.db.list_nodes()?.iter())?;
            for node_record in node_records.iter() {
                self.scan(node_record).await?;
                interval.tick().await;
                debug!("tick");
            }

            if node_records.is_empty() {
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
            sender: sender,
            message_rcvr: conn.subscribe_iob()
        }
    }

    async fn scan(&mut self, node: &NodeRecord) -> Result<(), Box<dyn std::error::Error>> {
        info!("Scan node {}", node.mac());

        let msg;
        {
            let mut buf = packet::buffer::Dynamic::new();
            PtNetPacket::with_asdh(&ptnet_c::ASDH::with(0x3E, COT::REQ, false), &mut buf)?
                .begin_asdu(&ptnet_c::DUI::with_direct(ptnet_c::TC_C_RD, 1, false))?
                .add_ioa(0)?
                .end_asdu()?;

            msg = Message {
                port: PORT_AUTO,
                header: ptnet_c::Header {
                    C: (BIT_PRM | FC_PRM_SEND_NOREPLY) as u8,
                    address: node.address,
                },
                payload: buf.into(),
            };

        }

        debug!("Transmit request");
        let rcvr = self.sender.send_message(&msg).await?;

        debug!("Await request result");
        let result = rcvr.await?;
        debug!("result = {}", result);

        let rsp: IOBMessage;
        {
            let timeout = sleep(Duration::from_secs(5));
            tokio::pin!(timeout);
            'rsp_loop: loop {
                select! {
                    msg = self.message_rcvr.recv() => {
                        rsp = msg?;
                        debug!("Some response arrived");

                        if NodeScanProcess::match_rsp_ti232(&rsp, node) {
                            break 'rsp_loop;
                        }
                        break;
                    },
                    _ = &mut timeout => {
                        warn!("Response timed out!");
                        return Ok(());
                    }
                }
            }
        }

        info!("Matching response arrived");

        Ok(())
    }

    fn match_rsp_ti232(rsp: &IOBMessage, node: &NodeRecord) -> bool {
        let IOBMessage { iob, message } = rsp;
        if message.header.address == node.address {
            if iob.asdh == ASDH::with(0x3E, COT::REQ, false) && iob.ioa == 1 {
                if let IE::TI232(_) = iob.ie {
                    return true;
                }
            }
        }

        false
    }
}