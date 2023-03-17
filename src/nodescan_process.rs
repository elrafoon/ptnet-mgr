use std::{time::Duration};
use async_trait::async_trait;

use log::{info, debug, warn};
use tokio::{time::{interval, sleep}, sync::broadcast, select};

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
    sender: &'a ClientConnectionSender<'a>,
    message_rcvr: broadcast::Receiver<Message>
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
            sender: sender,
            message_rcvr: conn.subscribe()
        }
    }

    async fn scan(&mut self, node: &NodeRecord) -> Result<(), Box<dyn std::error::Error>> {
        info!("Scan node {}", node.mac());

        let msg;
        {
            let mut buf = packet::buffer::Dynamic::new();
            PtNetPacket::with_asdh(&ptnet_c::ASDH::with(0, COT::REQ, false), &mut buf)?
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

        let rsp: Message;
        {
            let timeout = sleep(Duration::from_secs(5));
            tokio::pin!(timeout);
            'rsp_loop: loop {
                select! {
                    msg = self.message_rcvr.recv() => {
                        rsp = msg?;
                        debug!("Some response arrived");

                        if rsp.header.prm() && rsp.header.address == node.address {
                            if let Some(fc) = rsp.header.fc() {
                                match fc {
                                    FC::PrmSendConfirm | FC::PrmSendNoreply => { break 'rsp_loop; },
                                    _ => {}
                                }
                            }
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
}