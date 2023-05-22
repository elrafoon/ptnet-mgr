use std::{sync::Arc, io};

use redb::ReadableTable;
use serde::Serialize;

use super::{UpdateMode, node_table::{NodeRecord, NodeTable, self, NODE_TABLE}, NodeAddress, RawValue};

pub trait TableKey<K> {
    fn table_key(&self) -> &K
    where
        K: redb::RedbKey;
}

impl TableKey<NodeAddress> for NodeRecord {
    fn table_key(&self) -> &NodeAddress {
        &self.address
    }
}

pub trait DatabaseTable<T> {
    // type Key;
    type Record: Clone;
    type Event;

    fn redb(&self) -> &redb::Database;
    fn table_definition(&self) -> T;
    fn send_event(&self, evt: Self::Event);
    fn make_record_added_event(&self, rec: Self::Record) -> Self::Event;
    fn make_record_modified_event(&self, rec: Self::Record) -> Self::Event;
}

impl<'a> DatabaseTable<redb::TableDefinition<'static, &'static NodeAddress, &'static RawValue>> for NodeTable<'a> {
    type Record = NodeRecord;
    type Event = node_table::Event;

    fn redb(&self) -> &redb::Database {
        self.db
    }

    fn table_definition(&self) -> redb::TableDefinition<'static,&'static NodeAddress, &'static RawValue>
    {
        NODE_TABLE
    }

    fn send_event(&self, evt: Self::Event) {
        self.events.send(evt).unwrap_or_default();
    }

    fn make_record_added_event(&self, rec: Self::Record) -> Self::Event {
        Self::Event::NodeAdded(Arc::new(rec))
    }

    fn make_record_modified_event(&self, rec: Self::Record) -> Self::Event {
        Self::Event::NodeModified(Arc::new(rec))
    }
}

pub trait TableOps<'a,Key,Value,Record: Clone> {
    fn x_update_many<'t,T>(&self, it: T, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Iterator<Item = &'t Record> + Clone,
        Record: 't;
}


impl<'a,T,Key,Value,Record> TableOps<'a,Key,Value,Record> for T
where
    for<'t> T: DatabaseTable<redb::TableDefinition<'a, &'t Key, &'t Value>,Record=Record>,
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
{
    fn x_update_many<'t,IT>(&self, it: IT, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        IT: Iterator<Item = &'t Record> + Clone,
        Record: 't
    {
        let mut events: Vec<T::Event> = Vec::new();
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
