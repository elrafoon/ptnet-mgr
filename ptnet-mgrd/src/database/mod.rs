use serde::{Serialize, Deserialize};

use self::{node_table::{NodeTable, NODE_TABLE}};

pub mod node_table;
pub mod algo;

pub type RawNodeAddress = [u8; 6];

#[derive(Serialize,Deserialize,Clone,Copy,Debug,Default,PartialEq)]
pub struct NodeAddress {
    raw: RawNodeAddress
}

impl NodeAddress {
    pub fn as_raw(&self) -> RawNodeAddress {
        self.raw
    }
}

impl From<[u8; 6]> for NodeAddress {
    fn from(value: [u8; 6]) -> Self {
        NodeAddress { raw: value }
    }
}

impl Into<[u8; 6]> for NodeAddress {
    fn into(self) -> [u8; 6] {
        self.raw
    }
}

impl ToString for NodeAddress {
    fn to_string(&self) -> String {
        let a = &self.raw;
        format!("{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}",
            a.get(0).unwrap(), a.get(1).unwrap(), a.get(2).unwrap(),
            a.get(3).unwrap(), a.get(4).unwrap(), a.get(5).unwrap()
        )
    }
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
    pub(crate) inner_db: &'a redb::Database,
    pub nodes: NodeTable<'a>,
}

impl<'a> Database<'a> {
    pub fn new(re_db: &'a redb::Database) -> Self {
        Self {
            inner_db: re_db,
            nodes: NodeTable::new(&re_db),
        }
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let txn = self.inner_db.begin_write()?;
        {
            let _node_table = txn.open_table(NODE_TABLE)?;
            let _fwu_state_table = txn.open_table(FWU_STATE_TABLE)?;
        }
        txn.commit()?;

        Ok(())
    }
}
