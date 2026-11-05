//! State shared between the web server and the control loop.

use std::sync::{Arc, Mutex};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ControllerState {
    /// `None` until the DS18B20 is connected (later iteration).
    pub current_temp_c: Option<f32>,
    pub target_temp_c: f32,
    pub enabled: bool,
    /// Output duty in percent, 0..=100, written by the control loop.
    pub heater_duty: f32,
    pub fan_duty: f32,
    pub uptime_s: u64,
}

impl ControllerState {
    pub fn new(target_temp_c: f32) -> Self {
        Self {
            current_temp_c: None,
            target_temp_c,
            enabled: true,
            heater_duty: 0.0,
            fan_duty: 0.0,
            uptime_s: 0,
        }
    }
}

pub type SharedState = Arc<Mutex<ControllerState>>;
