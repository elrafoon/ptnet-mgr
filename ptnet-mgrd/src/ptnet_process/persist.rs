use tokio::sync::broadcast;
use async_trait::async_trait;
use ptnet::{IE};

use crate::{database::{Database}, client_connection::{ClientConnection, IOBMessage}};

use super::PtNetProcess;

pub struct PersistProcess<'a> {
    db: &'a Database<'a>,
    iob_rcvr: broadcast::Receiver<IOBMessage>
}

impl<'a> PersistProcess<'a> {
    pub fn new(db: &'a Database, conn: &'a ClientConnection) -> Self {
        PersistProcess {
            db: db,
            iob_rcvr: conn.subscribe_iob()
        }
    }

/*
    fn persist_prm(&self, msg: &Message) -> Result<(), E> {
        let scanner = Scanner::new(&msg.payload[..]);
        //for tok in scanner.ne
    }
*/
}

#[async_trait]
impl<'a> PtNetProcess for PersistProcess<'a> {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let IOBMessage { iob, message: msg } = self.iob_rcvr.recv().await?;

            if iob.asdh.ca == 0x3E {
                match iob.ioa {
                    1 => if let IE::TI232(ti232) = iob.ie {
                            self.db.nodes.modify(&msg.header.address, |opt_rec| {
                                let mut rec = opt_rec.unwrap_or_default();
                                rec.device_status = Some(ti232);
                                Some(rec)
                            })?;
                        },
                    2 => if let IE::TI233(ti233) = iob.ie {
                            self.db.nodes.modify(&msg.header.address, |opt_rec| {
                                let mut rec = opt_rec.unwrap_or_default();
                                rec.device_descriptor = Some(ti233);
                                Some(rec)
                            })?;
                        },
                    _ => ()
                }
            }
        }
    }
}
