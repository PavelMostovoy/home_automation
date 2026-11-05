//! PID configuration and controller for the heater.
//!
//! Input: temperature error (target - current) in °C. Output: heater duty 0..=100 %.

// ---------- PID configuration ----------
/// Proportional gain, % duty per °C of error.
pub const KP: f32 = 10.0;
/// Integral gain, % duty per (°C · s).
pub const KI: f32 = 0.1;
/// Derivative gain, % duty per (°C / s). Applied to the measurement, not the error,
/// so setpoint changes don't cause a kick.
pub const KD: f32 = 0.0;
/// Control loop period in milliseconds.
pub const PERIOD_MS: u64 = 1_000;
/// Output limits, % duty.
pub const OUT_MIN: f32 = 0.0;
pub const OUT_MAX: f32 = 100.0;

pub struct Pid {
    integral: f32,
    last_measurement: Option<f32>,
}

impl Pid {
    pub fn new() -> Self {
        Self { integral: 0.0, last_measurement: None }
    }

    /// Clears the integral and derivative history (e.g. when the controller is disabled).
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.last_measurement = None;
    }

    /// One PID step. `dt_s` is the time since the previous step in seconds.
    pub fn update(&mut self, target: f32, measurement: f32, dt_s: f32) -> f32 {
        let error = target - measurement;

        let derivative = match self.last_measurement {
            Some(last) if dt_s > 0.0 => -(measurement - last) / dt_s,
            _ => 0.0,
        };
        self.last_measurement = Some(measurement);

        // Anti-windup: integrate only while the output isn't saturated in the same direction.
        let new_integral = self.integral + error * dt_s;
        let output = KP * error + KI * new_integral + KD * derivative;
        if (output < OUT_MAX || error < 0.0) && (output > OUT_MIN || error > 0.0) {
            self.integral = new_integral;
        }

        (KP * error + KI * self.integral + KD * derivative).clamp(OUT_MIN, OUT_MAX)
    }
}
