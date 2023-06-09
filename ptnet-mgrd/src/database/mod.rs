use self::{node_table::{NodeTable, NODE_TABLE}, fwu_state_table::{FWU_STATE_TABLE, FWUStateTable}};

pub mod node_table;
pub mod fwu_state_table;
pub mod algo;

pub type NodeAddress = [u8; 6];
type RawValue = [u8];

pub fn node_address_to_string(a: &NodeAddress) -> String {
    format!("{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}:{:#02X}",
        a.get(0).unwrap(), a.get(1).unwrap(), a.get(2).unwrap(),
        a.get(3).unwrap(), a.get(4).unwrap(), a.get(5).unwrap()
    )
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
    pub fwu_state: FWUStateTable<'a>
}

impl<'a> Database<'a> {
    pub fn new(re_db: &'a redb::Database) -> Self {
        Self {
            inner_db: re_db,
            nodes: NodeTable::new(&re_db),
            fwu_state: FWUStateTable::new(&re_db)
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
