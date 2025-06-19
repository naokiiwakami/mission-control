# Analog3 Mission Control

## About This Project

TBD

## Getting Started

### Prerequisites

#### Install Development Tools:

- rust that supports edition 2024 or later
- gcc
- cmake

#### Install Required Libraries:
- libclang -> `sudo apt install libclang1`
- WiringPi -> see https://github.com/naokiiwakami/WiringPi

#### Enable SPI on Raspberry Pi

Start raspi-config by following command,
```
sudo raspi-config
```

then go to `Interface Options` -> `SPI`. Enable the SPI interface.

### Build the can-controller Submodule

Run the build script:

```
./build-can-controller.sh
```

### Run

```
cd mission-control
cargo run
```