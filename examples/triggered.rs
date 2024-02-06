use ina219::address::Address;
use ina219::configuration::{Configuration, MeasuredSignals, OperatingMode};
use ina219::SyncIna219;
use linux_embedded_hal::I2cdev;
use std::error::Error;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    let device = I2cdev::new("/dev/i2c-1")?;
    let mut ina = SyncIna219::new(device, Address::from_byte(0x42)?)?;

    ina.set_configuration(Configuration {
        // Only measure if we kindly ask
        operating_mode: OperatingMode::Triggered(MeasuredSignals::ShutAndBusVoltage),
        ..Configuration::default()
    })?;

    // Wait for the for measurement to be done
    let conversion_time: Duration = ina.configuration()?.conversion_time().unwrap();
    std::thread::sleep(conversion_time);

    // Writing the configuration started the first measurement
    let measurements = ina.next_measurement()?;
    println!("After configuration: {:?}", measurements);
    assert!(measurements.is_some());

    // If we wait and check again there will be no new data
    std::thread::sleep(conversion_time);
    let measurements = ina.next_measurement()?;
    println!("After no trigger: {:?}", measurements);
    assert!(measurements.is_none());

    // But we can start a new conversion using a trigger
    ina.trigger()?;

    std::thread::sleep(conversion_time);

    let measurements = ina.next_measurement()?;
    println!("After trigger: {:?}", measurements);
    assert!(measurements.is_some());

    Ok(())
}
