use ina219::address::Address;
use ina219::SyncIna219;
use linux_embedded_hal::I2cdev;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let device = I2cdev::new("/dev/i2c-1")?;
    let mut ina = SyncIna219::new(device, Address::from_byte(0x42)?)?;

    // Wait until a result is ready
    std::thread::sleep(ina.configuration()?.conversion_time().unwrap());

    println!("Bus Voltage: {}", ina.bus_voltage()?);
    println!("Shunt Voltage: {}", ina.shunt_voltage()?);

    Ok(())
}
