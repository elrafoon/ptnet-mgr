use std::{io, iter::Iterator, sync::Arc};

use redb::ReadableTable;
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;
use crate::ptnet::ptnet_c;

pub type NodeAddress = [u8; 6];
type RawValue = [u8];

pub fn node_address_to_string(a: &NodeAddress) -> String {
    format!("{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}",
        a.get(0).unwrap(), a.get(1).unwrap(), a.get(2).unwrap(),
        a.get(3).unwrap(), a.get(4).unwrap(), a.get(5).unwrap()
    )
}

const TABLE: redb::TableDefinition<&NodeAddress, &RawValue> = redb::TableDefinition::new("nodes");

#[derive(Debug,Serialize,Deserialize,Clone,Default,PartialEq)]
pub struct NodeRecord {
    pub address: NodeAddress,
    pub device_status: Option<ptnet_c::M_DEV_ST>,
    pub device_descriptor: Option<ptnet_c::M_DEV_DC>
}

impl NodeRecord {
    pub fn mac(&self) -> String {
        node_address_to_string(&self.address)
    }
}

#[derive(Clone)]
pub enum Event {
    NodeAdded(Arc<NodeRecord>),
    NodeModified(Arc<NodeRecord>),
}

pub enum UpdateMode {
    UpdateOrCreate,
    MustCreate,
    MustExist
}

impl Default for UpdateMode {
    fn default() -> Self { UpdateMode::UpdateOrCreate }
}

pub struct Database<'a> {
    db: &'a redb::Database,
    // TODO: Get rid of nodes, just access database
    // pub nodes: HashMap<NodeAddress, NodeRecord>,
    pub events: broadcast::Sender<Event>
}

impl<'a> Database<'a>
{
    pub fn new(db: &'a redb::Database) -> Self {
        let (evt_sender, _) = broadcast::channel::<Event>(128);

        Database {
            db: db,
            // nodes: HashMap::new(),
            events: evt_sender
        }
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_write()?;
        {
            let _table = txn.open_table(TABLE)?;
        }
        txn.commit()?;

        Ok(())
    }

    pub fn count_nodes(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(TABLE)?;
        Ok(table.len()? as usize)
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeAddress>, Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(TABLE)?;
        let mut results: Vec<NodeAddress> = Vec::new();
        results.reserve_exact(table.len()? as usize);
        for entry in table.iter()? {
            let (item, _) = entry?;
            results.push(item.value().clone());
        }
        Ok(results)
    }

    pub fn load_nodes<'call, T: Iterator<Item = &'call NodeAddress>>(&self, iter: T) -> Result<Vec<NodeRecord>, Box<dyn std::error::Error>> {
        // pub fn remove_nodes<'call, T: Iterator<Item = &'call NodeAddress>>(&self, iter: T) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(TABLE)?;
        let mut results: Vec<NodeRecord> = Vec::new();

        for address in iter {
            match table.get(address)? {
                Some(cbor) => {
                    let rec: NodeRecord = serde_cbor::from_slice(cbor.value()).unwrap();
                    results.push(rec);
                },
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Node {} does not exist", node_address_to_string(address))
                    )));
                }
            }
        }

        Ok(results)
    }

    /// Modify node in callback
    pub fn modify_node<T>(&self, address: &NodeAddress, cb: T) -> Result<(), Box<dyn std::error::Error>>
    where
        T: FnOnce(Option<NodeRecord>) -> Option<NodeRecord>
    {
        let event: Option<Event>;
        let txn = self.db.begin_write()?;

        {
            let mut table = txn.open_table(TABLE)?;
            let rec: Option<NodeRecord> = match table.get(address)? {
                None => None,
                Some(cbor) => Some(serde_cbor::from_slice(cbor.value()).unwrap())
            };

            match cb(rec) {
                None => return Ok(()),
                Some(rec) => {
                    match table.insert(address, serde_cbor::to_vec(&rec)?.as_slice())? {
                        None => event = Some(Event::NodeAdded(Arc::new(rec))),
                        Some(_) => event = Some(Event::NodeModified(Arc::new(rec)))
                    };
                }
            }
        }

        txn.commit()?;

        if let Some(evt) = event {
            self.events.send(evt).unwrap_or_default();
        }

        Ok(())
    }

    /// update or create node
    pub fn update_node(&self, address: &NodeAddress, rec: &NodeRecord, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>> {
        let prev_rec_exists;

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE)?;

            match mode {
                UpdateMode::MustCreate => {
                    if table.get(address)?.is_some() {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!("Node {} already exists", rec.mac())
                        )));
                    }
                },
                UpdateMode::MustExist => {
                    if table.get(address)?.is_none() {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::NotFound,
                            format!("Node {} does not exist", rec.mac())
                        )));
                    }
                },
                UpdateMode::UpdateOrCreate => {}
            };

            let rec_cbor = serde_cbor::to_vec(rec)?;
            let rec_bytes = rec_cbor.as_slice();
            prev_rec_exists = table.insert(address, rec_bytes)?.is_some();
        }

        txn.commit()?;

        self.events.send(
            match prev_rec_exists {
                false => Event::NodeAdded(Arc::new(rec.clone())),
                true => Event::NodeModified(Arc::new(rec.clone()))
            }
        ).unwrap_or_default();
        Ok(())
    }

    pub fn remove_nodes<'call, T: Iterator<Item = &'call NodeAddress>>(&self, iter: T) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE)?;
            for address in iter {
                table.remove(address)?;
            }
        }
        txn.commit()?;
        Ok(())
    }

    pub fn update_nodes<'b,T>(&mut self, it: T, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Iterator<Item = &'b NodeRecord> + Clone,
    {
        let mut events: Vec<Event> = Vec::new();
        // let prev_rec_exists;

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE)?;

            for rec in it {
                match mode {
                    UpdateMode::MustCreate => {
                        if table.get(&rec.address)?.is_some() {
                            return Err(Box::new(io::Error::new(
                                io::ErrorKind::AlreadyExists,
                                format!("Node {} already exists", rec.mac())
                            )));
                        }
                    },
                    UpdateMode::MustExist => {
                        if table.get(&rec.address)?.is_none() {
                            return Err(Box::new(io::Error::new(
                                io::ErrorKind::NotFound,
                                format!("Node {} does not exist", rec.mac())
                            )));
                        }
                    },
                    UpdateMode::UpdateOrCreate => {}
                };

                let rec_cbor = serde_cbor::to_vec(rec)?;
                let rec_bytes = rec_cbor.as_slice();
                let prev_rec = table.insert(&rec.address, rec_bytes)?;

                events.push(
                    match prev_rec {
                        None => Event::NodeAdded(Arc::new(rec.clone())),
                        Some(_) => Event::NodeModified(Arc::new(rec.clone()))
                    }
                );
            }
        }
        txn.commit()?;

        while let Some(evt) = events.pop() {
            self.events.send(evt).unwrap_or_default();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, str::FromStr};

    use futures::FutureExt;

    use crate::ptnet::ptnet_c::{M_DEV_ST, FW_Version_A, HW_Version_A, M_DEV_DC};

    use super::*;

    #[test]
    fn node_events() {
        let rdb = make_redb();
        let db = make_db(&rdb);
        let mut rcvr = db.events.subscribe();

        let mut rec = NodeRecord {
            address: [0xFE, 0xED, 0xDE, 0xAF, 0xBE, 0xEF],
            device_status: Some(M_DEV_ST {
                fw_state: 2,
                fw_version: FW_Version_A {
                    major: 1,
                    minor: 2,
                    patch: 3
                },
                hw_version: HW_Version_A {
                    vid: 0x80,
                    pid: 0x86,
                    rev: 0x11,
                },
            }),
            device_descriptor: None
        };

        db.update_node(&rec.address, &rec, UpdateMode::MustCreate).expect("update_node shall succeeed");

        let evt = rcvr.recv().now_or_never().expect("Event shall arrive").unwrap();
        if let Event::NodeAdded(n_rec) = evt {
            assert_eq!(rec, *n_rec);
        } else {
            assert!(false, "NodeAdded event not generated");
        }

        assert!(rcvr.is_empty(), "Exactly one event should have been generated");

        rec.device_descriptor = Some(M_DEV_DC {
            b: [1,0,0,0,0,0,0]
        });

        db.update_node(&rec.address, &rec, UpdateMode::MustExist).unwrap();

        let evt = rcvr.recv().now_or_never().expect("Event shall arrive").unwrap();
        if let Event::NodeModified(n_rec) = evt {
            assert_eq!(rec, *n_rec);
        } else {
            assert!(false, "NodeModified event not generated");
        }

        assert!(rcvr.is_empty(), "Exactly one event should have been generated");
    }

    fn make_redb() -> redb::Database {
        let pth = PathBuf::from_str("test-db.redb").unwrap();
        fs::remove_file(&pth).unwrap_or_default();
        redb::Database::create(&pth).unwrap()
    }

    fn make_db<'a>(redb_db: &'a redb::Database) -> Database<'a> {
        let mut db = Database::new(redb_db);
        db.init().unwrap();
        db
    }
}
