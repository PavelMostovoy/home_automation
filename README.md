# home_automation — ESP32-S3 thermostat

Rust firmware (`esp-idf-svc`, `std`) for an ESP32-S3. The board starts its own Wi-Fi
access point and serves a web UI for the thermostat.

| Setting       | Value                                  |
|---------------|----------------------------------------|
| Target        | `xtensa-esp32s3-espidf`                |
| ESP-IDF       | `v5.3.4` (project-local, `.embuild/`)  |
| Wi-Fi SSID    | `Thermostat`                           |
| Wi-Fi pass    | `thermo1234`                           |
| Web UI        | <http://192.168.71.1/>                 |

You can change the SSID, password and setpoint limits in `src/config.rs`.

---

## 1. One-time prerequisites (user-installed, global)

You install these yourself once. The project never installs or changes them.

| Tool                    | Location                       | Purpose                                         |
|-------------------------|--------------------------------|-------------------------------------------------|
| `esp` Rust toolchain    | `~/.rustup/toolchains/esp`     | Xtensa Rust compiler (`rust-toolchain.toml` → `channel = "esp"`) |
| `~/export-esp.sh`       | home directory                 | Sets `LIBCLANG_PATH` and adds `xtensa-esp-elf` GCC to `PATH` |
| `espflash`              | `~/.cargo/bin`                 | Flashes the firmware and opens the serial monitor |
| `ldproxy`               | `~/.cargo/bin`                 | Linker wrapper used by `esp-idf-sys`             |

If something is missing, install it with:

```sh
cargo install espup && espup install   # esp toolchain + generates ~/export-esp.sh
cargo install espflash ldproxy
```

You don't need to install ESP-IDF separately. On the first build, `esp-idf-sys`/`embuild`
downloads it (~3 GB) into `./.embuild/espressif/`, inside the project. This is set by
`ESP_IDF_TOOLS_INSTALL_DIR = "workspace"` in `.cargo/config.toml`.

---

## 2. Before starting RustRover

RustRover (cargo, rust-analyzer, bindgen) needs the Xtensa environment, so **start RustRover
from a shell where `~/export-esp.sh` has already been sourced**:

```sh
source ~/export-esp.sh
open -a RustRover /Users/pmostovoi/RustroverProjects/home_automation
```

A Dock or Spotlight launch doesn't inherit these variables, and builds will then fail
(for example, bindgen can't find `libclang`).

To check the environment:

```sh
echo $LIBCLANG_PATH          # should point into ~/.rustup/toolchains/esp/.../esp-clang/lib
which xtensa-esp32s3-elf-gcc # should resolve inside ~/.rustup/toolchains/esp/xtensa-esp-elf
```

The same applies to a plain terminal: run `source ~/export-esp.sh` once per shell session
before any `cargo` command.

---

## 3. Build

```sh
cd /Users/pmostovoi/RustroverProjects/home_automation
source ~/export-esp.sh

cargo build            # debug
cargo build --release  # release (opt-level "s", smaller/faster)
```

The first build is slow because it downloads and compiles ESP-IDF into `.embuild/`.
Cargo also writes its crate cache to `~/.cargo/registry`.

The binary ends up in:

- `target/xtensa-esp32s3-espidf/debug/home_automation`
- `target/xtensa-esp32s3-espidf/release/home_automation`

---

## 4. Flash the firmware to the device

1. Connect the ESP32-S3 over USB. Use the board's USB/UART port. Data cable, not charge-only.
2. Find the serial port, if you need to:

   ```sh
   ls /dev/cu.usb*
   ```

3. Build, flash and open the serial monitor in one step. The cargo runner is
   `espflash flash --monitor`:

   ```sh
   source ~/export-esp.sh
   cargo run --release
   ```

   Or flash an already-built binary directly:

   ```sh
   espflash flash --monitor target/xtensa-esp32s3-espidf/release/home_automation
   # pick the port explicitly if several are connected:
   espflash flash --monitor --port /dev/cu.usbmodem1101 target/xtensa-esp32s3-espidf/release/home_automation
   ```

4. Exit the monitor with `Ctrl+C`. To reopen the monitor later without reflashing:

   ```sh
   espflash monitor
   ```

**If flashing fails** with "failed to connect" or the port isn't found, put the board in
download mode: hold **BOOT**, press and release **RESET** (EN), then release **BOOT** and
retry. Close any other serial monitor (RustRover, `screen`, etc.) that is holding the port.

To fully erase the flash, for example to clear old NVS data:

```sh
espflash erase-flash
```

---

## 5. Use it

After boot, the serial log shows:

```
AP 'Thermostat' up, web UI at http://192.168.71.1/
```

Connect a phone or laptop to the `Thermostat` Wi-Fi network (password `thermo1234`) and
open <http://192.168.71.1/>.

---

## Hardware wiring (Arduino Nano ESP32, ABX00083)

The code uses GPIO numbers; the board silkscreen shows Arduino labels.
Official pinout: `documentation/ABX00083-full-pinout.pdf`.

| Function       | Board pin | GPIO | Notes |
|----------------|-----------|------|-------|
| DS18B20 data   | A3        | 4    | 4.7 kΩ pull-up to **3V3** (never 5 V); sensor VDD on 3V3 |

---

## Project layout

```
src/
  main.rs      entry point
  config.rs    compile-time config (AP credentials, setpoint limits, planned pins)
  wifi.rs      soft access point
  web.rs       HTTP server
  web/index.html  web UI
  state.rs     thermostat state
  ds18b20.rs   DS18B20 sensor on A3/GPIO4 (bit-banged 1-Wire)
.cargo/config.toml   target, runner, ESP-IDF version, project-local tool install
rust-toolchain.toml  uses the `esp` toolchain
tmp/                 scratch scripts and findings (git-ignored)
```
