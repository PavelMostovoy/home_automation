//! ESP32-S3 thermostat. Iteration 1: Wi-Fi access point + web UI/API.
//! Sensor (DS18B20), PID and MOSFET outputs come in later iterations.

mod config;
mod state;
mod web;
mod wifi;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;

use crate::state::ControllerState;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let p = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let state = Arc::new(Mutex::new(ControllerState::new(config::DEFAULT_TARGET_C)));

    let _wifi = wifi::start_access_point(p.modem, sys_loop, nvs)?;
    let _server = web::start(state.clone())?;

    // Placeholder for the future control loop; keeps `_wifi` and `_server` alive.
    let started = Instant::now();
    loop {
        state.lock().unwrap().uptime_s = started.elapsed().as_secs();
        std::thread::sleep(Duration::from_secs(1));
    }
}
