fn main() {
    // MAC of the ESP32 gateway this S3 node talks to over ESP-NOW.
    let gateway_mac =
        std::env::var("GATEWAY_MAC").unwrap_or_else(|_| "FF:FF:FF:FF:FF:FF".to_string());
    println!("cargo:rustc-env=GATEWAY_MAC={gateway_mac}");
    println!("cargo:rerun-if-env-changed=GATEWAY_MAC");
    embuild::espidf::sysenv::output();
}
