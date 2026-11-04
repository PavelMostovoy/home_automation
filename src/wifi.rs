//! Soft access point. Clients get an IP via DHCP; the board is at 192.168.71.1.

use anyhow::{Result, anyhow};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AccessPointConfiguration, AuthMethod, BlockingWifi, Configuration, EspWifi};

use crate::config;

pub fn start_access_point(
    modem: Modem<'static>,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> Result<BlockingWifi<EspWifi<'static>>> {
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sys_loop.clone(), Some(nvs))?, sys_loop)?;

    let auth_method = if config::AP_PASSWORD.is_empty() {
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };

    wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
        ssid: config::AP_SSID.try_into().map_err(|_| anyhow!("SSID too long"))?,
        password: config::AP_PASSWORD.try_into().map_err(|_| anyhow!("password too long"))?,
        auth_method,
        channel: config::AP_CHANNEL,
        max_connections: config::AP_MAX_CONNECTIONS,
        ..Default::default()
    }))?;

    wifi.start()?;
    wifi.wait_netif_up()?;

    let ip = wifi.wifi().ap_netif().get_ip_info()?;
    log::info!("AP '{}' up, web UI at http://{}/", config::AP_SSID, ip.ip);
    Ok(wifi)
}
