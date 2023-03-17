pub(crate) fn parse_user_address(node_address: &str) -> Option<[u8; 6]> {
    let mut uid: Vec<u8> = node_address.split(":").map(|x| u8::from_str_radix(x, 16).unwrap()).collect();
    uid.insert(0, 0);
    uid.insert(0, 0);
    uid.try_into().ok()
}