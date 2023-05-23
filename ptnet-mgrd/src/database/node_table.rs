use std::{sync::Arc, io};

use ptnet;
use redb::ReadableTable;
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

use super::{UpdateMode, NodeAddress};

pub(super) const NODE_TABLE: redb::TableDefinition<NodeAddress, NodeRecord> = redb::TableDefinition::new("nodes");

#[derive(Debug,Serialize,Deserialize,Clone,Default,PartialEq)]
pub struct NodeRecord {
    pub address: NodeAddress,
    pub device_status: Option<ptnet::M_DEV_ST>,
    pub device_descriptor: Option<ptnet::M_DEV_DC>
}

impl NodeRecord {
    pub fn mac(&self) -> String {
        self.address.to_string()
    }
}

impl redb::RedbValue for NodeRecord {
    type SelfType<'a> = NodeRecord
    where
        Self: 'a;

    type AsBytes<'a> = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a
    {
        serde_cbor::from_slice(data).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b
    {
        serde_cbor::to_vec(value).unwrap()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("NodeRecord")
    }
}

#[derive(Clone)]
pub enum Event {
    NodeAdded(Arc<NodeRecord>),
    NodeModified(Arc<NodeRecord>),
}

pub struct NodeTable<'a> {
    pub(crate) db: &'a redb::Database,
    pub events: broadcast::Sender<Event>
}

impl<'a> NodeTable<'a> {
    pub fn new(db: &'a redb::Database) -> Self {
        let (evt_sender, _) = broadcast::channel::<Event>(128);

        Self {
            db: db,
            events: evt_sender
        }
    }

    pub fn len(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(NODE_TABLE)?;
        Ok(table.len()? as usize)
    }

    pub fn list(&self) -> Result<Vec<NodeAddress>, Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(NODE_TABLE)?;
        let mut results: Vec<NodeAddress> = Vec::new();
        results.reserve_exact(table.len()? as usize);
        for entry in table.iter()? {
            let (item, _) = entry?;
            results.push(item.value().clone());
        }
        Ok(results)
    }

    pub fn load_many<'call, T: Iterator<Item = &'call NodeAddress>>(&self, iter: T) -> Result<Vec<NodeRecord>, Box<dyn std::error::Error>> {
        // pub fn remove_nodes<'call, T: Iterator<Item = &'call NodeAddress>>(&self, iter: T) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(NODE_TABLE)?;
        let mut results: Vec<NodeRecord> = Vec::new();

        for address in iter {
            match table.get(address)? {
                Some(rec) => results.push(rec.value()),
                None => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Node {} does not exist", address.to_string())
                    )));
                }
            }
        }

        Ok(results)
    }

    /// Modify node in callback
    pub fn modify<T>(&self, address: &NodeAddress, cb: T) -> Result<(), Box<dyn std::error::Error>>
    where
        T: FnOnce(Option<NodeRecord>) -> Option<NodeRecord>
    {
        let event: Option<Event>;
        let txn = self.db.begin_write()?;

        {
            let mut table = txn.open_table(NODE_TABLE)?;
            let rec: Option<NodeRecord> = match table.get(address)? {
                None => None,
                Some(rec) => Some(rec.value())
            };

            match cb(rec) {
                None => return Ok(()),
                Some(rec) => {
                    match table.insert(address, rec.clone())? {
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
    pub fn update(&self, address: &NodeAddress, rec: &NodeRecord, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>> {
        let prev_rec_exists;

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(NODE_TABLE)?;

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

            prev_rec_exists = table.insert(address, rec)?.is_some();
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

    pub fn remove_many<'call, T: Iterator<Item = &'call NodeAddress>>(&self, iter: T) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(NODE_TABLE)?;
            for address in iter {
                table.remove(address)?;
            }
        }
        txn.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, str::FromStr};

    use futures::FutureExt;
    use ptnet::{M_DEV_ST, FW_Version_A, HW_Version_A, M_DEV_DC};

    use crate::database::Database;

    use super::*;

    #[test]
    fn node_events() {
        let rdb = make_redb();
        let db = make_db(&rdb);
        let mut rcvr = db.nodes.events.subscribe();

        let mut rec = NodeRecord {
            address: NodeAddress::from([0xFE, 0xED, 0xDE, 0xAF, 0xBE, 0xEF]),
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

        db.nodes.update(&rec.address, &rec, UpdateMode::MustCreate).expect("update_node shall succeeed");

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

        db.nodes.update(&rec.address, &rec, UpdateMode::MustExist).unwrap();

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
