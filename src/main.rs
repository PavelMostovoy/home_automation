//! ESP32-S3 thermostat (Arduino Nano ESP32).
//! Wi-Fi access point + web UI/API, DS18B20 temperature reading.

mod config;
mod ds18b20;
mod state;
mod web;
mod wifi;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;

use crate::ds18b20::Ds18b20;
use crate::state::{ControllerState, SharedState};

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let p = Peripherals::take()?;

    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let state = Arc::new(Mutex::new(ControllerState::new(config::DEFAULT_TARGET_C)));

    // DS18B20 data -> GPIO4 = A3. Own thread: a reading blocks for ~750 ms.
    let sensor = Ds18b20::new(p.pins.gpio4)?;
    let st = state.clone();
    std::thread::Builder::new()
        .name("ds18b20".into())
        .stack_size(4096)
        .spawn(move || sensor_task(sensor, st))?;

    let _wifi = wifi::start_access_point(p.modem, sys_loop, nvs)?;
    let _server = web::start(state.clone())?;

    // Keeps `_wifi` and `_server` alive.
    let started = Instant::now();
    loop {
        state.lock().unwrap().uptime_s = started.elapsed().as_secs();
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Reads the DS18B20 forever and publishes the temperature into the shared state.
/// After `SENSOR_MAX_FAILURES` consecutive errors the temperature is cleared (`None`).
fn sensor_task(mut sensor: Ds18b20, state: SharedState) {
    match sensor.read_rom() {
        Ok(rom) => log::info!("DS18B20 found, ROM {:02X?}", rom),
        Err(e) => log::warn!("DS18B20 not detected at start: {e}"),
    }

    let period = Duration::from_millis(config::SENSOR_PERIOD_MS);
    let mut failures = 0u32;
    loop {
        let started = Instant::now();
        match sensor.read_temperature() {
            Ok(t) => {
                if failures > 0 {
                    log::info!("DS18B20 recovered after {failures} failed read(s)");
                }
                failures = 0;
                log::info!("Temperature: {t:.5} °C");
                state.lock().unwrap().current_temp_c = Some(t);
            }
            Err(e) => {
                failures += 1;
                log::warn!("DS18B20 read failed ({failures}/{}): {e}", config::SENSOR_MAX_FAILURES);
                if failures >= config::SENSOR_MAX_FAILURES {
                    state.lock().unwrap().current_temp_c = None;
                }
            }
        }
        std::thread::sleep(period.saturating_sub(started.elapsed()).max(Duration::from_millis(100)));
    }
}
