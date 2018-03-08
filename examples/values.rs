extern crate i2cdev;
extern crate ina219;

use i2cdev::linux::{LinuxI2CDevice};
use ina219::{INA219, INA219_ADDR};

fn main() {
    let device = LinuxI2CDevice::new("/dev/i2c-1", INA219_ADDR as u16).unwrap();
    let mut ina = INA219::new(device);

    ina.calibrate(&[0x10, 0x00]).unwrap();

    let voltage = ina.voltage().unwrap();
    println!("bus voltage: {:?}", voltage);

    let shunt_voltage = ina.shunt_voltage().unwrap();
    println!("shunt voltage: {:?}", shunt_voltage);

    let current = ina.current().unwrap();
    println!("current: {:?}", current);

    let power = ina.power().unwrap();
    println!("power: {:?}", power);
}
