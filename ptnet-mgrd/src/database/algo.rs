use std::{sync::Arc, io, mem::size_of};

use redb::ReadableTable;
use serde::Serialize;

use super::{UpdateMode, node_table::{NodeRecord, NodeTable, self, NODE_TABLE}, RawNodeAddress, NodeAddress};

impl redb::RedbValue for NodeAddress {
    type SelfType<'a> = NodeAddress
    where
        Self: 'a;

    type AsBytes<'a> = &'a [u8]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> { Some(size_of::<RawNodeAddress>()) }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a
    {
        NodeAddress {
            raw: data.try_into().expect("Slice len match RawNodeAddress length")
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b
    {
        &value.raw
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("NodeAddress")
    }
}

impl redb::RedbKey for NodeAddress {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}


pub trait TableKey<K> {
    fn table_key<'k>(&self) -> K::SelfType<'k>
    where
        K: redb::RedbKey + 'k;
}

impl TableKey<NodeAddress> for NodeRecord {
    fn table_key<'k>(&self) -> NodeAddress
    where
        NodeAddress: 'k
    {
        NodeAddress { raw: self.address.into() }
    }
}

pub trait DatabaseTable {
    type Key: redb::RedbKey;
    type Record: redb::RedbValue + Clone;
    type Event;

    fn redb(&self) -> &redb::Database;
    fn table_definition(&self) -> &redb::TableDefinition<Self::Key, Self::Record>;
    fn send_event(&self, evt: Self::Event);
    fn make_record_added_event(&self, rec: Self::Record) -> Self::Event;
    fn make_record_modified_event(&self, rec: Self::Record) -> Self::Event;
}

impl<'a> DatabaseTable for NodeTable<'a> {
    type Key = NodeAddress;
    type Record = NodeRecord;
    type Event = node_table::Event;

    fn redb(&self) -> &redb::Database {
        self.db
    }

    fn table_definition(&self) -> &redb::TableDefinition<'static,NodeAddress,NodeRecord> {
        &NODE_TABLE
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

pub trait TableOps<T, Key, Record>
where
    T: DatabaseTable<Key=Key, Record=Record>,
    Key: redb::RedbKey,
    Record: redb::RedbValue + TableKey<Key> + Clone + Serialize,
{
    fn update_many<'t,IT>(&self, it: IT, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        IT: Iterator<Item = &'t T::Record> + Clone,
        T::Record: 't,
        &'t Record: std::borrow::Borrow<<Record as redb::RedbValue>::SelfType<'t>>;
}

impl<T, Key, Record> TableOps<T, Key, Record> for T
where
    T: DatabaseTable<Key=Key, Record=Record>,
    Key: redb::RedbKey + 'static,
    Record: redb::RedbValue + TableKey<Key> + Clone + Serialize + 'static,
{
    fn update_many<'t,IT>(&self, it: IT, mode: UpdateMode) -> Result<(), Box<dyn std::error::Error>>
    where
        IT: Iterator<Item = &'t Record> + Clone,
        Record: 't,
        &'t Record: std::borrow::Borrow<<Record as redb::RedbValue>::SelfType<'t>>
    {
        let mut events: Vec<T::Event> = Vec::new();

        let txn = self.redb().begin_write()?;
        {
            let mut table = txn.open_table(*self.table_definition())?;

            for rec in it {
                let rec_key = rec.table_key();
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

                let prev_rec = table.insert(rec.table_key(), rec)?;

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
