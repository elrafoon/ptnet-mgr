use serde::{Deserialize};

#[derive(Clone,Debug,Deserialize)]
pub struct UserModel {
    pub network: Option<Network>
}

#[derive(Clone,Debug,Deserialize)]
pub struct Network {
    pub ballasts: Vec<Ballast>,
    pub sensors: Vec<Sensor>
}

pub type NodeId = String;
pub type TypeId = String;

#[derive(Clone,Debug,Deserialize)]
pub struct Ballast {
    pub address: NodeId,
    #[serde(rename="type")]
    pub type_id: TypeId,
    pub name: String,
}

#[derive(Clone,Debug,Deserialize)]
pub struct Sensor {
    pub address: NodeId,
    pub type_id: TypeId,
    pub name: String,
    pub part_of: Option<NodeId>
}
