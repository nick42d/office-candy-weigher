# Office Candy Weigher

## Required parts
- Load cell (e.g https://core-electronics.com.au/makerverse-10kg-load-cell.html) 
- HX710 load cell ADC (e.g https://core-electronics.com.au/makerverse-load-cell-amplifier.html)
- Rasbperry Pi Pico 1 (Pico 1 WH was used, but any of the Pico 1 range should work. The H version is useful as it has a connector soldered onboard to the debug pins)
- ST7789 based colour LCD display (e.g https://core-electronics.com.au/pimoroni-pico-display-pack.html)
- 4x momentary buttons (note - built into display pack above)
- RGB LED (note - built into display pack above)
### Simulation only
- Rotary encoder (e.g https://core-electronics.com.au/encoder-module-with-button.html)
### Deployment only
- Pico debug probe or equivalent (e.g https://core-electronics.com.au/raspberry-pi-debug-probe.html)

## Configuration
Configurable variables are located in `src/config_consts.rs`.

## How to deploy
This project uses `probe-rs` to deploy via the inbuilt debug pins on the pico.

1. Install the rust toolchain (see https://doc.rust-lang.org/book/ch01-01-installation.html if you're unsure about this).
1. Install and setup `probe-rs` via the guide here - https://probe.rs/docs/getting-started/. Note - udev rules were required to be setup to get my environment working.
1. Attach the debug probe to the debug port/pins on the pico, and connect both the probe and pico to your dev machine via USB.
1. Deploy and run via `cargo r --release`! Whilst running with the probe connected, debug output is available in the terminal.

That's it - the app is now installed on the microcontroller and it will run automatically on boot.

### Simulation
To enable the rotary encoder as simulator, deploy and run via `cargo r --release -F hardware-sim`

To enable the software simulator, deploy and run via `cargo r --release -F software-sim`

### Note
This was developed on Linux - it should also be able to deploy via Windows however it's not been tested.
