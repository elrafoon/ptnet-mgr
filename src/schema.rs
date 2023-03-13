use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/schema/sol.model.rs"));
include!(concat!(env!("OUT_DIR"), "/schema/sol.user.rs"));