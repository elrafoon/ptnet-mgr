use std::{path::PathBuf, fs};

use log::info;

use crate::{database::node_table::NodeRecord, sol::schema};

fn parse_user_address(node_address: &str) -> Option<[u8; 6]> {
    let mut uid: Vec<u8> = node_address.split(":").map(|x| u8::from_str_radix(x, 16).unwrap()).collect();
    uid.insert(0, 0);
    uid.insert(0, 0);
    uid.try_into().ok()
}

pub fn load(model_root: &str) -> Result<Vec<NodeRecord>, std::io::Error> {
    let mut sol_user_path = PathBuf::from(model_root);
    sol_user_path.push("sol.user.json");
    info!("Loading SOL user model from {}", sol_user_path.as_os_str().to_str().unwrap());
    let soluser: schema::UserModel = serde_json::from_reader(fs::File::open(sol_user_path)?)?;
    info!("Model loaded");

    if let Some(network) = soluser.network.as_ref() {
        let mut nodes: Vec<NodeRecord> =
            network.ballasts.iter()
                .map(|ballast| parse_user_address(ballast.address.as_str()).unwrap())
                .map(|address| NodeRecord { address: address, ..Default::default() })
                .collect();

        nodes.extend(
            network.sensors.iter()
                .filter(|e| e.part_of.is_none())
                .map(|sensor| parse_user_address(sensor.address.as_str()).unwrap())
                .map(|address| NodeRecord { address: address, ..Default::default() })
        );

        Ok(nodes)
    } else {
        Ok(Vec::<NodeRecord>::new())
    }
}
