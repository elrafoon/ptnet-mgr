use std::{sync::Arc, io};

use ptnet;
use redb::ReadableTable;
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

use super::{NodeAddress, RawValue, node_address_to_string, UpdateMode};

pub(super) const NODE_TABLE: redb::TableDefinition<&NodeAddress, &RawValue> = redb::TableDefinition::new("nodes");

#[derive(Debug,Serialize,Deserialize,Clone,Default,PartialEq)]
pub struct NodeRecord {
    pub address: NodeAddress,
    pub device_status: Option<ptnet::M_DEV_ST>,
    pub device_descriptor: Option<ptnet::M_DEV_DC>
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

    pub fn update_many<'b,T>(&mut self, it: T, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Iterator<Item = &'b NodeRecord> + Clone,
    {
        let mut events: Vec<Event> = Vec::new();
        // let prev_rec_exists;

        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(NODE_TABLE)?;

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

/*

pub trait TableKey<K> {
    fn table_key(&self) -> &K
    where
        K: redb::RedbKey;
}

/*
impl TableKey<NodeAddress> for NodeRecord {
    fn table_key(&self) -> NodeAddress {
        self.address
    }
}
*/

impl TableKey<NodeAddress> for NodeRecord {
    fn table_key(&self) -> &NodeAddress {
        &self.address
    }
}


/*

pub trait TableKey<K>
where K: redb::RedbKey {
    fn table_key(&self) -> K;
}

impl<'a> TableKey<&'a NodeAddress> for NodeRecord {
    fn table_key(&self) -> &NodeAddress {
        &self.address
    }
}
*/

pub trait DatabaseTable<T> {
    // type Key;
    type Record: Clone;
    type Event;

    fn redb(&self) -> &redb::Database;
    fn table_definition(&self) -> T;
    /*
    fn open_table<'db,'txn,'key>(&self, txn: &'txn redb::WriteTransaction<'db>) -> Result<redb::Table<'db,'txn,Self::Key,&'static RawValue>,redb::Error>
    where
        Self::Key: redb::RedbKey + 'key;
    */
    fn send_event(&self, evt: Event);
    fn make_record_added_event(&self, rec: Self::Record) -> Event;
    fn make_record_modified_event(&self, rec: Self::Record) -> Event;
}

impl<'a> DatabaseTable<redb::TableDefinition<'static, &'static NodeAddress, &'static RawValue>> for NodeTable<'a> {
    // type Key = &'static NodeAddress;
    type Record = NodeRecord;
    type Event = Event;

    fn redb(&self) -> &redb::Database {
        self.db
    }

    fn table_definition(&self) -> redb::TableDefinition<'static,&'static NodeAddress, &'static RawValue>
    {
        NODE_TABLE
    }

    /*
    fn table_definition(&self) -> redb::TableDefinition<Self::Key,&RawValue>
    where
        Self::Key: redb::RedbKey
    {
        NODE_TABLE
    }

    fn open_table<'db,'txn,'key>(&self, txn: &'txn redb::WriteTransaction<'db>) -> Result<redb::Table<'db,'txn,Self::Key,&'static RawValue>,redb::Error>
    where
        Self::Key: redb::RedbKey + 'key
    {
        txn.open_table(NODE_TABLE)
    }
        */

    fn send_event(&self, evt: Event) {
        self.events.send(evt).unwrap_or_default();
    }

    fn make_record_added_event(&self, rec: Self::Record) -> Event {
        Event::NodeAdded(Arc::new(rec))
    }

    fn make_record_modified_event(&self, rec: Self::Record) -> Event {
        Event::NodeModified(Arc::new(rec))
    }
}

pub trait TableOps<'a,Key,Value,Record: Clone> {
    // type Record: Clone;
    // type Key;

    fn x_update_many<'t,T>(&self, it: T, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Iterator<Item = &'t Record> + Clone,
        Record: 't;
        // Self::Record: TableKey<Self::Key> + Serialize + 'b,
        // Self::Key: redb::RedbKey + 'b;
}


pub fn x_update_many<'t,T,IT,Key,Record>(dt: T, it: IT, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
where
    T: Borrow<DatabaseTable<redb::TableDefinition<'t, &'t Key, &'t RawValue>>,
    IT: Iterator<Item = &'t Record> + Clone,
    Record: TableKey<Key> + Serialize + 't,
    Key: redb::RedbKey + 'static,
    &'t Key: redb::RedbKey + 'static,
    for<'a> &'a Key: std::borrow::Borrow<Key>
{
    let mut events: Vec<Event> = Vec::new();
    // let prev_rec_exists;

    let txn = dt.redb().begin_write()?;
    {
        let mut table = txn.open_table(dt.table_definition())?;
        // let mut table = self.open_table(&txn)?;

        for rec in it {
            let rec_key = rec.table_key();
            match mode {
                UpdateMode::MustCreate => {
                    if table.get(rec_key)?.is_some() {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            "Record already exists"
                        )));
                    }
                },
                UpdateMode::MustExist => {
                    if table.get(&rec_key)?.is_none() {
                        return Err(Box::new(io::Error::new(
                            io::ErrorKind::NotFound,
                            "Record does not exist"
                        )));
                    }
                },
                UpdateMode::UpdateOrCreate => {}
            };

            let rec_cbor = serde_cbor::to_vec(rec)?;
            let rec_bytes = rec_cbor.as_slice();
            let prev_rec = table.insert(rec_key, rec_bytes)?;

            events.push(
                match prev_rec {
                    None => dt.make_record_added_event(rec),
                    Some(_) => dt.make_record_modified_event(rec)
                }
            );
        }
    }
    txn.commit()?;

    while let Some(evt) = events.pop() {
        table.send_event(evt);
    }

    Ok(())
}

#[cfg(kokot)]
impl<'a,T,Key,Value,Record> TableOps<'a,Key,Value,Record> for T
where
    for<'t> T: DatabaseTable<redb::TableDefinition<'a, &'t Key, &'t Value>,Record=Record>,
    //for<'t> Key: redb::RedbKey + std::borrow::Borrow<Key::SelfType<'t>> + 't,
    for<'t> &'t Key: redb::RedbKey,
    for<'t> &'t Value: redb::RedbValue,
    Record: Serialize + Clone,
    for<'t> &'t Record: TableKey<Key>,
    Key: redb::RedbKey + 'static,
    Key: Copy,
    for<'t> &'t Key: std::borrow::Borrow<<&'t Key as redb::RedbValue>::SelfType<'t>>,
    Value: 'static,
    for<'t> Key: std::borrow::Borrow<<&'t Key as redb::RedbValue>::SelfType<'t>>,
    for<'t> &'t [u8]: std::borrow::Borrow<<&'t Value as redb::RedbValue>::SelfType<'t>>
    // for<'a> &'a Value: std::borrow::Borrow<Value::SelfType<'a>>,
    // for<'t> Key: redb::RedbKey + std::borrow::Borrow<Key::SelfType<'t>> + 'static,
    /*
    for<'a> &'a Key: std::borrow::Borrow<Key::SelfType<'a>>,
    for<'a> &'a Value: std::borrow::Borrow<Value::SelfType<'a>>,
    for<'a> &'a [u8]: std::borrow::Borrow<Value::SelfType<'a>>
    */
{
    // type Key = T::Key;

    fn x_update_many<'t,IT>(&self, it: IT, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        IT: Iterator<Item = &'t Record> + Clone,
        Record: 't
        // Self::Record: TableKey<Key> + Serialize + 'b
    {
        let mut events: Vec<Event> = Vec::new();
        // let prev_rec_exists;

        let txn = self.redb().begin_write()?;
        {
            let mut table = txn.open_table(self.table_definition())?;
            // let mut table = self.open_table(&txn)?;

            for rec in it {
                let rec_key = *rec.table_key();
                match mode {
                    UpdateMode::MustCreate => {
                        if table.get(&rec_key)?.is_some() {
                            return Err(Box::new(io::Error::new(
                                io::ErrorKind::AlreadyExists,
                                "Record already exists"
                            )));
                        }
                    },
                    UpdateMode::MustExist => {
                        if table.get(&rec_key)?.is_none() {
                            return Err(Box::new(io::Error::new(
                                io::ErrorKind::NotFound,
                                "Record does not exist"
                            )));
                        }
                    },
                    UpdateMode::UpdateOrCreate => {}
                };

                let rec_cbor = serde_cbor::to_vec(rec)?;
                let rec_bytes = rec_cbor.as_slice();
                let prev_rec = table.insert(*rec.table_key(), rec_bytes)?;

                events.push(
                    match prev_rec {
                        None => self.make_record_added_event(rec.clone()),
                        Some(_) => self.make_record_modified_event(rec.clone())
                    }
                );
            }
        }
        txn.commit()?;

        while let Some(evt) = events.pop() {
            self.send_event(evt);
        }

        Ok(())
    }
}

*/

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
