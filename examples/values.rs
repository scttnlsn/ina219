extern crate ina219;
extern crate linux_embedded_hal as hal;

use hal::I2cdev;
use ina219::{INA219, INA219_ADDR};

fn main() {
    let device = I2cdev::new("/dev/i2c-1").unwrap();
    let mut ina = INA219::new(device, INA219_ADDR).unwrap();

    ina.calibrate(0x0100).unwrap();

    let voltage = ina.bus_voltage().unwrap();
    println!("bus voltage: {:?}", voltage);

    let shunt_voltage = ina.shunt_voltage().unwrap();
    println!("shunt voltage: {:?}", shunt_voltage);

    let current = ina.current_raw().unwrap();
    println!("current: {:?}", current);

    let power = ina.power_raw().unwrap();
    println!("power: {:?}", power);
}
