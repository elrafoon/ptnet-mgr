use std::sync::Arc;

use ptnet::image_header::FWVersion;
use redb::ReadableTable;
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

use super::{NodeAddress, RawValue};

pub(super) const FWU_STATE_TABLE: redb::TableDefinition<&NodeAddress, &RawValue> = redb::TableDefinition::new("fwu_state");

#[derive(Debug,Serialize,Deserialize,Clone,PartialEq,Default)]
pub enum Goal {
    #[default]
    None,
    /// keep current fw version
    KeepCurrent,
    /// ask user if it's ok to update
    ApproveUpdateTo(FWVersion),
    /// user confirmed update, perform it
    UpdateTo(FWVersion)
}

#[derive(Debug,Serialize,Deserialize,Clone,Default,PartialEq)]
pub struct FWUStateRecord {
    pub goal: Goal
}

#[derive(Clone)]
pub enum Event {
    FWUStateAdded(Arc<FWUStateRecord>),
    FWUStateModified(Arc<FWUStateRecord>)
}

pub struct FWUStateTable<'a> {
    db: &'a redb::Database,
    pub events: broadcast::Sender<Event>
}

impl<'a> FWUStateTable<'a> {
    pub fn new(db: &'a redb::Database) -> Self {
        let (evt_sender, _) = broadcast::channel::<Event>(128);

        Self {
            db: db,
            events: evt_sender
        }
    }

    pub fn get_or_create_for(&self, address: &NodeAddress) -> Result<FWUStateRecord, Box<dyn std::error::Error>> {
        let txn = self.db.begin_write()?;

        let mut table = txn.open_table(FWU_STATE_TABLE)?;

        if let Some(cbor) = table.get(address)? {
            // no need to commit
            return Ok(serde_cbor::from_slice(cbor.value()).unwrap());
        }

        let def_rec = FWUStateRecord::default();
        table.insert(address, serde_cbor::to_vec(&def_rec)?.as_slice())?;

        drop(table);

        txn.commit()?;
        Ok(def_rec)
    }

    /// Modify state record in callback
    pub fn modify<T>(&self, address: &NodeAddress, cb: T) -> Result<(), Box<dyn std::error::Error>>
    where
        T: FnOnce(Option<FWUStateRecord>) -> Option<FWUStateRecord>
    {
        let event: Option<Event>;
        let txn = self.db.begin_write()?;

        {
            let mut table = txn.open_table(FWU_STATE_TABLE)?;
            let rec: Option<FWUStateRecord> = match table.get(address)? {
                None => None,
                Some(cbor) => Some(serde_cbor::from_slice(cbor.value()).unwrap())
            };

            match cb(rec) {
                None => return Ok(()),
                Some(rec) => {
                    match table.insert(address, serde_cbor::to_vec(&rec)?.as_slice())? {
                        None => event = Some(Event::FWUStateAdded(Arc::new(rec))),
                        Some(_) => event = Some(Event::FWUStateModified(Arc::new(rec)))
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

}
