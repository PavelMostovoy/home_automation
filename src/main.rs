//! ESP32-S3 thermostat (Arduino Nano ESP32).
//! Wi-Fi access point + web UI/API, DS18B20 temperature reading,
//! PID heater control (D3) with a heater-following fan (D2).

mod config;
mod ds18b20;
mod pid;
mod state;
mod web;
mod wifi;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::nvs::EspDefaultNvsPartition;

use crate::ds18b20::Ds18b20;
use crate::pid::Pid;
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

    // Fan MOSFET gate -> GPIO5 = D2. 10 kHz PWM, 12-bit resolution.
    let fan_timer = LedcTimerDriver::new(
        p.ledc.timer0,
        &TimerConfig::new().frequency(Hertz(10_000)).resolution(Resolution::Bits12),
    )?;
    let fan = LedcDriver::new(p.ledc.channel0, &fan_timer, p.pins.gpio5)?;

    // Heater MOSFET gate -> GPIO6 = D3. 10 Hz PWM (slow switching keeps the MOSFET cool),
    // 14-bit resolution (low frequencies need more bits for the LEDC divider to fit).
    let heater_timer = LedcTimerDriver::new(
        p.ledc.timer1,
        &TimerConfig::new().frequency(Hertz(10)).resolution(Resolution::Bits14),
    )?;
    let heater = LedcDriver::new(p.ledc.channel1, &heater_timer, p.pins.gpio6)?;

    // Control loop: PID -> heater duty, fan duty follows the heater (20..100 %).
    let st = state.clone();
    std::thread::Builder::new()
        .name("control".into())
        .stack_size(8192) // float formatting in log! needs more than 4 KB
        .spawn(move || {
            // Keep the timers alive: dropping a timer resets it.
            let _timers = (fan_timer, heater_timer);
            control_task(heater, fan, st)
        })?;

    let _wifi =wifi::start_access_point(p.modem, sys_loop, nvs)?;
    let _server = web::start(state.clone())?;

    // Keeps `_wifi` and `_server` alive.
    let started = Instant::now();
    loop {
        state.lock().unwrap().uptime_s = started.elapsed().as_secs();
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Fan duty for a given heater duty: heater 0..100 % maps linearly to fan 20..100 %,
/// so the fan never stops completely.
fn fan_duty_for(heater_pct: f32) -> f32 {
    config::FAN_MIN_DUTY + (100.0 - config::FAN_MIN_DUTY) * heater_pct / 100.0
}

/// Sets a PWM output to `pct` percent (0..=100).
fn set_pct(pwm: &mut LedcDriver<'_>, pct: f32) {
    let duty = (pwm.get_max_duty() as f32 * pct.clamp(0.0, 100.0) / 100.0).round() as u32;
    if let Err(e) = pwm.set_duty(duty) {
        log::warn!("PWM set_duty failed: {e}");
    }
}

/// Runs the PID every `pid::PERIOD_MS` and drives heater + fan.
/// Heater is off (fan at minimum) when disabled or when there is no valid temperature.
fn control_task(mut heater: LedcDriver<'static>, mut fan: LedcDriver<'static>, state: SharedState) {
    let mut pid = Pid::new();
    let period = Duration::from_millis(pid::PERIOD_MS);
    let mut last = Instant::now();
    loop {
        std::thread::sleep(period);
        let dt_s = last.elapsed().as_secs_f32();
        last = Instant::now();

        let (temp, target, enabled) = {
            let s = state.lock().unwrap();
            (s.current_temp_c, s.target_temp_c, s.enabled)
        };

        let heater_pct = match temp {
            Some(t) if enabled => pid.update(target, t, dt_s),
            _ => {
                pid.reset();
                0.0
            }
        };
        let fan_pct = fan_duty_for(heater_pct);

        set_pct(&mut heater, heater_pct);
        set_pct(&mut fan, fan_pct);
        log::debug!("Heater {heater_pct:.1}%  Fan {fan_pct:.1}%");

        let mut s = state.lock().unwrap();
        s.heater_duty = heater_pct;
        s.fan_duty = fan_pct;
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
                log::debug!("Temperature: {t:.5} °C");
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
