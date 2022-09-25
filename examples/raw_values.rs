extern crate linux_embedded_hal as hal;

extern crate ina219_rs as ina219;

use hal::I2cdev;

use ina219::ina219::{INA219,Opts};

fn main() {

    let device = I2cdev::new("/dev/i2c-1").unwrap();
	
    let opt = Opts::default();
    let mut ina = INA219::new(device,opt);
    ina.init().unwrap();

    let voltage = ina.voltage().unwrap();
    println!("bus voltage: {:?}",voltage);

    let voltage_raw = ina.voltage_raw().unwrap();
    println!("bus voltage_raw: {:?}mV",voltage_raw);

    let shunt = ina.shunt_voltage().unwrap();
    println!("shunt voltage: {:?}",shunt);

    let current = ina.current().unwrap();
    println!("current: {:?}",current);

    let current_raw = ina.current_raw().unwrap();
    println!("current_raw: {:?}",current_raw);

    let power = ina.power().unwrap();
    println!("power: {:?}",power);

}