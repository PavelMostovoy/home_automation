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

// Planned pins for later iterations (ESP32-S3, avoiding strapping/USB/flash pins):
// DS18B20 data -> GPIO4 (4.7k pull-up), heater MOSFET -> GPIO5, fan MOSFET -> GPIO6
