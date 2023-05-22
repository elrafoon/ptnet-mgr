use std::sync::Arc;

use async_trait::async_trait;
use log::{error, info};
use ptnet::{FW_State_A, FC, PtNetPacket, ASDHConstruct, COT, DUIConstruct, FW_Version_A};
use tokio::sync::broadcast;

use crate::{database::{Database, node_table::{self, NodeRecord, Event::{NodeAdded, NodeModified}}, fwu_state_table::Goal}, client_connection::{ClientConnection, ClientConnectionSender}, fw_index::FirmwareIndex};

use super::PtNetProcess;

pub struct FWUProcess<'a> {
    db: &'a Database<'a>,
    conn: &'a ClientConnection,
    sender: &'a ClientConnectionSender<'a>,
    fw_index: &'a FirmwareIndex,
    node_evt_rcvr: broadcast::Receiver<node_table::Event>
}

impl<'a> FWUProcess<'a> {
    pub fn new(db: &'a Database, conn: &'a ClientConnection, sender: &'a ClientConnectionSender<'a>, fw_index: &'a FirmwareIndex) -> Self {
        let fwu = Self {
            db: db,
            conn: conn,
            sender: sender,
            fw_index: fw_index,
            node_evt_rcvr: db.nodes.events.subscribe()
        };

        return fwu;
    }

    async fn process_node(&self, node: &NodeRecord) -> Result<(), Box<dyn std::error::Error>> {
        let fwu_state = self.db.fwu_state.get_or_create_for(&node.address)?;
        // if device_status is not known, it's impossible to do anything with this node
        if let Some(device_status) = node.device_status {
            let fw_state: FW_State_A = device_status.fw_state.try_into()?;
            match fwu_state.goal {
                Goal::None => {
                    match fw_state {
                        FW_State_A::Idle => {
                            if let Some(fws) = self.fw_index.get_firmwares_for(&device_status.hw_version.into()) {
                                // get latest firmware
                                if let Some((latest_ver, _)) = fws.last_key_value() {
                                    // is firmware newer than currently running on node?
                                    if *latest_ver > device_status.fw_version.into() {
                                        // yes, it's newer
                                        info!("Newer firmware {} available for node '{}'", latest_ver, node.mac());

                                        // self.db.fwu_state.modify(address, cb)
                                    }
                                }
                            }
                        },
                        FW_State_A::Download | FW_State_A::Flashing | FW_State_A::Updated => {
                            info!("cancel firmware update on '{}' in progress, since it's non-goal", node.mac());

                            let mut buf = packet::buffer::Dynamic::new();

                            PtNetPacket::with_asdh(&ptnet::ASDH::with(0x3E, COT::DEACT, false), &mut buf)?
                                .begin_asdu(&ptnet::DUI::with_direct(ptnet::TC_C_FW_IU, 1, false))?
                                .add_ioa(0)?
                                .end_asdu()?;

                            if let Err(err) = self.sender.send_prm(FC::PrmSendNoreply, &node.address, &buf).await {
                                error!("Error sending TI240 to '{}'! ({})", node.mac(), err);
                            }
                        },
                    }
                },
                Goal::KeepCurrent => todo!(),
                Goal::ApproveUpdateTo(ver) => todo!(),
                Goal::UpdateTo(ver) => todo!(),
            }
        }
        Ok(())
    }
}

#[async_trait]
impl<'a> PtNetProcess for FWUProcess<'a> {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let evt = self.node_evt_rcvr.recv().await?;

            match evt {
                NodeAdded(node) | NodeModified(node) => {
                    if let Err(err) = self.process_node(&node).await {
                        error!("Error processing node '{}'! ({})", node.mac(), err);
                    }
                }
            }
        }
    }
}