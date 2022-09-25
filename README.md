# ina219

[![crates.io](https://img.shields.io/crates/v/ina219_rs.svg)](https://crates.io/crates/ina219_rs)

[INA219](http://www.ti.com/product/INA219) current/power monitor driver for Rust

## Example

```bash
cargo build --example values --target=aarch64-unknown-linux-musl
cargo build --example raw_values --target=aarch64-unknown-linux-musl
```

## add this line to Cargo.toml

```toml
ina219_rs = "0.2.1"
```

```rust
extern crate linux_embedded_hal as hal;

extern crate ina219_rs as ina219;

use hal::I2cdev;
use ina219::physic;

use ina219::ina219::{INA219,Opts};

fn main() {

    let device = I2cdev::new("/dev/i2c-1").unwrap();
    let opt = Opts::new(0x42,100 * physic::MilliOhm,1 * physic::Ampere);
    //let opt = Opts::default();
    let mut ina = INA219::new(device,opt);
    ina.init().unwrap();
    let pm = ina.sense().unwrap();
    println!("{:?}",pm);
 /* output
 Debug: PowerMonitor
{
        Voltage = 8.228V,
        Shunt_Voltage = 534µV,
        Current = 1.750A,
        Power = 744mW
}
 */

```
