use esp_idf_svc::espnow::{EspNow, PeerInfo};
use esp_idf_svc::sys::{
    esp_wifi_set_channel, esp_wifi_set_max_tx_power, esp_wifi_set_ps, wifi_ps_type_t_WIFI_PS_NONE,
    wifi_second_chan_t_WIFI_SECOND_CHAN_NONE,
};

pub const ESPNOW_CHANNEL: u8 = 1;
const MIN_TX_POWER_DBM: i8 = 2;
const MAX_TX_POWER_DBM: i8 = 20;

pub fn lock_wifi_channel(channel: u8) {
    esp_idf_svc::sys::esp!(unsafe {
        esp_wifi_set_channel(channel, wifi_second_chan_t_WIFI_SECOND_CHAN_NONE)
    })
    .expect("esp_wifi_set_channel");
}

pub fn disable_wifi_power_save() {
    esp_idf_svc::sys::esp_nofail!(unsafe { esp_wifi_set_ps(wifi_ps_type_t_WIFI_PS_NONE) });
}

pub fn set_max_tx_power_dbm(dbm: i8) {
    let clamped = dbm.clamp(MIN_TX_POWER_DBM, MAX_TX_POWER_DBM);
    if clamped != dbm {
        log::warn!(
            "TX_POWER_DBM={} is outside ESP32 supported range; using {} dBm",
            dbm,
            clamped
        );
    }

    // ESP-IDF expects Wi-Fi TX power in 0.25 dBm units.
    let quarter_dbm = clamped.saturating_mul(4);
    esp_idf_svc::sys::esp!(unsafe { esp_wifi_set_max_tx_power(quarter_dbm) })
        .expect("esp_wifi_set_max_tx_power");
    log::info!("ESP-NOW max TX power set to {} dBm", clamped);
}

pub fn add_gateway_peer(
    esp_now: &EspNow<'_>,
    gateway_mac: [u8; 6],
) -> Result<(), esp_idf_svc::sys::EspError> {
    if esp_now.peer_exists(gateway_mac).unwrap_or(false) {
        esp_now.del_peer(gateway_mac)?;
    }

    let mut peer = PeerInfo::default();
    peer.peer_addr = gateway_mac;
    peer.channel = ESPNOW_CHANNEL;
    peer.encrypt = false;
    esp_now.add_peer(peer)
}
