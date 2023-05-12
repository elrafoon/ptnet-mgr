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

#[derive(Clone,Debug,Deserialize)]
pub struct Ballast {
    pub address: String,
    #[serde(rename="type")]
    pub type_id: String,
    pub name: String,
}

#[derive(Clone,Debug,Deserialize)]
pub struct Sensor {
    pub address: String,
    pub type_id: String,
    pub name: String,
    pub part_of: Option<String>
}
