use esp_idf_svc::espnow::{EspNow, PeerInfo, BROADCAST};
use esp_idf_svc::sys::{esp_wifi_set_ps, wifi_ps_type_t_WIFI_PS_NONE};

pub const ESPNOW_CHANNEL: u8 = 1;

pub fn disable_wifi_power_save() {
    esp_idf_svc::sys::esp_nofail!(unsafe { esp_wifi_set_ps(wifi_ps_type_t_WIFI_PS_NONE) });
}

pub fn add_gateway_peer(esp_now: &EspNow<'_>, gateway_mac: [u8; 6]) {
    if esp_now.peer_exists(gateway_mac).unwrap_or(false) {
        let _ = esp_now.del_peer(gateway_mac);
    }
    if esp_now.peer_exists(BROADCAST).unwrap_or(false) {
        let _ = esp_now.del_peer(BROADCAST);
    }

    let mut peer = PeerInfo::default();
    peer.peer_addr = gateway_mac;
    peer.channel = ESPNOW_CHANNEL;
    peer.encrypt = false;
    esp_now.add_peer(peer).expect("add gateway peer");

    let mut bcast = PeerInfo::default();
    bcast.peer_addr = BROADCAST;
    bcast.channel = ESPNOW_CHANNEL;
    bcast.encrypt = false;
    esp_now.add_peer(bcast).expect("add broadcast peer");
}
