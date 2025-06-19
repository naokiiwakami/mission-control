# Analog3 Mission Control

## Prerequisites

Install development tools:

- rust that supports edition 2024 or later
- gcc
- cmake

Install libraries:
- libclang -> `sudo apt install libclang1`
- WiringPi -> see https://github.com/naokiiwakami/WiringPi

Enable SPI on Raspberry Pi:

Start raspi-config:
```
sudo raspi-config
```

then go to `Interface Options` -> `SPI`. Enable the SPI interface.