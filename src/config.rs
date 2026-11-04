//! Compile-time configuration.

// ---------- Wi-Fi access point ----------
pub const AP_SSID: &str = "Thermostat";
/// WPA2 password, must be >= 8 chars. Set to "" for an open network.
pub const AP_PASSWORD: &str = "thermo1234";
pub const AP_CHANNEL: u8 = 6;
pub const AP_MAX_CONNECTIONS: u16 = 4;

// ---------- Setpoint ----------
pub const DEFAULT_TARGET_C: f32 = 25.0;
pub const MIN_TARGET_C: f32 = 5.0;
pub const MAX_TARGET_C: f32 = 80.0;

// ---------- Temperature sensor (see ds18b20.rs) ----------
/// Time between readings (a 12-bit conversion itself takes ~750 ms).
pub const SENSOR_PERIOD_MS: u64 = 1_000;
/// Consecutive failed reads before the temperature is cleared (`None`).
pub const SENSOR_MAX_FAILURES: u32 = 3;

// Arduino Nano ESP32 (ABX00083), see documentation/ABX00083-full-pinout.pdf.
// Code uses GPIO numbers, not board labels.
//   DS18B20 data -> GPIO4 = A3  (4.7k pull-up to 3V3, NOT 5V)
