fn main() {
    let gateway_mac =
        std::env::var("GATEWAY_MAC").unwrap_or_else(|_| "FF:FF:FF:FF:FF:FF".to_string());
    let node_id = std::env::var("NODE_ID").unwrap_or_else(|_| "1".to_string());
    println!("cargo:rustc-env=GATEWAY_MAC={gateway_mac}");
    println!("cargo:rustc-env=NODE_ID={node_id}");
    println!("cargo:rerun-if-env-changed=GATEWAY_MAC");
    println!("cargo:rerun-if-env-changed=NODE_ID");
    embuild::espidf::sysenv::output();
}
