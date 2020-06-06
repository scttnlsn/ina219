# ina219

[![Travis CI Status](https://travis-ci.org/scttnlsn/ina219.svg?branch=master)](https://travis-ci.org/scttnlsn/ina219)
[![crates.io](https://img.shields.io/crates/v/ina219.svg)](https://crates.io/crates/ina219)

[INA219](http://www.ti.com/product/INA219) current/power monitor driver for Rust

## Example

```rust
extern crate ina219;
extern crate linux_embedded_hal as hal;

use hal::I2cdev;
use ina219::{INA219, INA219_ADDR};

fn main() {
    let device = I2cdev::new("/dev/i2c-1").unwrap();
    let mut ina = INA219::new(device, INA219_ADDR);

    ina.calibrate(0x0100).unwrap();

    let voltage = ina.voltage().unwrap();
    println!("bus voltage: {:?}", voltage);

    let shunt_voltage = ina.shunt_voltage().unwrap();
    println!("shunt voltage: {:?}", shunt_voltage);

    let current = ina.current().unwrap();
    println!("current: {:?}", current);

    let power = ina.power().unwrap();
    println!("power: {:?}", power);
}

```
