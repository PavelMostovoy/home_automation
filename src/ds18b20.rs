//! DS18B20 temperature sensor on a bit-banged 1-Wire bus.
//!
//! Wiring (Arduino Nano ESP32): DQ -> A3 = GPIO4, VDD -> 3V3, GND -> GND,
//!
//! Single sensor on the bus: commands use SKIP ROM. Each timing-critical slot runs with
//! interrupts disabled (≤ ~70 µs) so Wi-Fi interrupts can't stretch the pulses.

use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{InputOutput, InputPin, OutputPin, PinDriver, Pull};
use esp_idf_svc::hal::interrupt;

const CMD_READ_ROM: u8 = 0x33;
const CMD_SKIP_ROM: u8 = 0xCC;
const CMD_CONVERT_T: u8 = 0x44;
const CMD_READ_SCRATCHPAD: u8 = 0xBE;

const FAMILY_DS18B20: u8 = 0x28;
/// Scratchpad value after power-up, before the first conversion finished (= 85.0 °C).
const RAW_POWER_ON_RESET: i16 = 0x0550;
/// 12-bit conversion takes up to 750 ms.
const CONVERSION_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct Ds18b20 {
    pin: PinDriver<'static, InputOutput>,
}

impl Ds18b20 {
    pub fn new(pin: impl InputPin + OutputPin + 'static) -> Result<Self> {
        // Open drain: we only ever pull low; the 4.7k resistor pulls high.
        // The internal pull-up (~45k) is only a weak backup, it does NOT replace the 4.7k.
        let mut pin = PinDriver::input_output_od(pin, Pull::Up)?;
        pin.set_high()?; // release the bus
        Ok(Self { pin })
    }

    /// Reads the 64-bit ROM code (only valid with a single device on the bus).
    pub fn read_rom(&mut self) -> Result<[u8; 8]> {
        self.reset()?;
        self.write_byte(CMD_READ_ROM)?;
        let mut rom = [0u8; 8];
        for b in rom.iter_mut() {
            *b = self.read_byte()?;
        }
        if crc8(&rom[..7]) != rom[7] {
            bail!("ROM CRC mismatch: {}", hex(&rom));
        }
        if rom[0] != FAMILY_DS18B20 {
            bail!("unexpected family code 0x{:02X} (DS18B20 is 0x28), ROM {}", rom[0], hex(&rom));
        }
        Ok(rom)
    }

    /// Starts a conversion, waits for it to finish and returns the temperature in °C.
    /// Blocks for up to ~750 ms.
    pub fn read_temperature(&mut self) -> Result<f32> {
        self.reset()?;
        self.write_byte(CMD_SKIP_ROM)?;
        self.write_byte(CMD_CONVERT_T)?;

        // The sensor holds the bus low (reads 0) while converting, then reads 1.
        let started = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(10));
            if self.read_bit()? {
                break;
            }
            if started.elapsed() > CONVERSION_TIMEOUT {
                bail!("conversion timeout");
            }
        }

        self.reset()?;
        self.write_byte(CMD_SKIP_ROM)?;
        self.write_byte(CMD_READ_SCRATCHPAD)?;
        let mut sp = [0u8; 9];
        for b in sp.iter_mut() {
            *b = self.read_byte()?;
        }
        decode_scratchpad(&sp)
    }

    // ---------- 1-Wire link layer (standard-speed timings, µs) ----------

    /// Reset pulse + presence detection.
    fn reset(&mut self) -> Result<()> {
        if !self.pin.is_high() {
            bail!("bus is held low (short to GND or missing 4.7k pull-up?)");
        }
        // Reset low time is a minimum (480 µs), so it may run with interrupts enabled.
        self.pin.set_low()?;
        Ets::delay_us(480);
        let presence = interrupt::free(|| -> Result<bool> {
            self.pin.set_high()?;
            Ets::delay_us(70);
            Ok(self.pin.is_low())
        })?;
        Ets::delay_us(410);
        if !presence {
            bail!("no presence pulse (sensor not connected / wrong pin / no power?)");
        }
        Ok(())
    }

    fn write_bit(&mut self, bit: bool) -> Result<()> {
        interrupt::free(|| -> Result<()> {
            self.pin.set_low()?;
            if bit {
                Ets::delay_us(6);
                self.pin.set_high()?;
                Ets::delay_us(64);
            } else {
                Ets::delay_us(60);
                self.pin.set_high()?;
                Ets::delay_us(10);
            }
            Ok(())
        })
    }

    fn read_bit(&mut self) -> Result<bool> {
        let bit = interrupt::free(|| -> Result<bool> {
            self.pin.set_low()?;
            Ets::delay_us(3);
            self.pin.set_high()?;
            Ets::delay_us(10);
            Ok(self.pin.is_high())
        })?;
        Ets::delay_us(55);
        Ok(bit)
    }

    fn write_byte(&mut self, byte: u8) -> Result<()> {
        for i in 0..8 {
            self.write_bit(byte & (1 << i) != 0)?; // LSB first
        }
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8> {
        let mut byte = 0u8;
        for i in 0..8 {
            if self.read_bit()? {
                byte |= 1 << i;
            }
        }
        Ok(byte)
    }
}

// ---------- pure helpers (no hardware) ----------

/// Validates the 9-byte scratchpad and converts it to °C.
fn decode_scratchpad(sp: &[u8; 9]) -> Result<f32> {
    if crc8(&sp[..8]) != sp[8] {
        bail!("scratchpad CRC mismatch: {}", hex(sp));
    }
    // All-zero data has a valid CRC (0) -> also check the config register's fixed bits.
    if sp[4] & 0x9F != 0x1F {
        bail!("invalid config register 0x{:02X}: {}", sp[4], hex(sp));
    }
    let raw = i16::from_le_bytes([sp[0], sp[1]]); // signed: negative temps work
    if raw == RAW_POWER_ON_RESET {
        bail!("power-on reset value (85.0 °C), conversion not done yet");
    }
    Ok(raw as f32 / 16.0)
}

/// Dallas/Maxim CRC-8 (polynomial x^8 + x^5 + x^4 + 1, reflected 0x8C).
fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in data {
        let mut b = byte;
        for _ in 0..8 {
            let mix = (crc ^ b) & 0x01;
            crc >>= 1;
            if mix != 0 {
                crc ^= 0x8C;
            }
            b >>= 1;
        }
    }
    crc
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}
