use std::{collections::HashMap, io, iter::Iterator};

use redb::ReadableTable;
use serde::{Serialize, Deserialize};
use crate::ptnet::ptnet_c;

type NodeAddress = [u8; 6];
type RawValue = [u8];

const TABLE: redb::TableDefinition<&NodeAddress, &RawValue> = redb::TableDefinition::new("nodes");

#[derive(Debug,Serialize,Deserialize,Clone,Default)]
pub struct NodeRecord {
    pub address: NodeAddress,
    pub device_status: Option<ptnet_c::M_DEV_ST>
}

impl NodeRecord {
    pub fn mac(&self) -> String {
        let a = &self.address;
        format!("{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}",
            a.get(0).unwrap(), a.get(1).unwrap(), a.get(2).unwrap(),
            a.get(3).unwrap(), a.get(4).unwrap(), a.get(5).unwrap()
        )
    }
}

pub struct Database<'a> {
    db: &'a redb::Database,
    pub nodes: HashMap<NodeAddress, NodeRecord>
}

impl<'a> Database<'a> {
    pub fn new(db: &'a redb::Database) -> Self {
        Database {
            db: db,
            nodes: HashMap::new()
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

    pub fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_read()?;
        match txn.open_table(TABLE) {
            Ok(table) => {
                for x in table.iter()? {
                    self.nodes.insert(x.0.value().clone(), serde_cbor::from_slice(x.1.value())?);
                }
                Ok(())
            },
            Err(err) => match err {
                redb::Error::TableDoesNotExist(_) => Ok(()),
                _ => Err(Box::new(err))
            }
        }
    }

    pub fn load_node(&self, address: &NodeAddress) -> Result<Option<NodeRecord>, redb::Error> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(TABLE)?;
        let rec = table.get(address)?;
        match rec {
            None =>
                Ok(None),
            Some(cbor) =>
                Ok(Some(serde_cbor::from_slice(cbor.value()).unwrap()))
        }
    }

    pub fn save_node(&self, address: &NodeAddress, rec: &NodeRecord) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(TABLE)?;
            let cbor = serde_cbor::to_vec(rec)?;
            let rec = cbor.as_slice();
            table.insert(address, rec)?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn add_node(&mut self, rec: &NodeRecord) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(_node) = self.nodes.get(&rec.address) {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Node {} already exists", rec.mac())
            )));
        }

        self.save_node(&rec.address, rec)?;
        self.nodes.insert(rec.address, rec.clone());
        Ok(())
    }

    pub fn add_nodes<'b,T>(&mut self, it: T) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Iterator<Item = &'b NodeRecord> + Clone,
    {
        for rec in it.clone() {
            if let Some(_node) = self.nodes.get(&rec.address) {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("Node {} already exists", rec.mac())
                )));
            }
        }

        let txn = self.db.begin_write()?;
        for node in it {
            let mut table = txn.open_table(TABLE)?;
            let cbor = serde_cbor::to_vec(node)?;
            let rec = cbor.as_slice();
            table.insert(&node.address, rec)?;
            self.nodes.insert(node.address, node.clone());
        }
        txn.commit()?;
        Ok(())
    }
}
