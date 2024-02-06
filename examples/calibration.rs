use ina219::address::Address;
use ina219::calibration::{IntCalibration, MicroAmpere};
use ina219::SyncIna219;
use linux_embedded_hal::I2cdev;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Resolution of 1A, and a shunt of 1mOhm
    let calib = IntCalibration::new(MicroAmpere(1_000_000), 1_000).unwrap();

    let device = I2cdev::new("/dev/i2c-1")?;
    let mut ina = SyncIna219::new_calibrated(device, Address::from_byte(0x42)?, calib)?;

    let measurement = ina.next_measurement()?.expect("A measurement is ready");

    println!("{:#?}", measurement);

    let err = (measurement.current.0 / 1_000_000)
        - i64::from(measurement.shunt_voltage.shunt_voltage_mv());
    assert!(err.abs() < 10);

    let err = (measurement.power.0 / 1_000_000).abs_diff(
        measurement.current.0 / 1_000_000 * i64::from(measurement.bus_voltage.voltage_mv() / 1000),
    );
    assert!(err < 100);

    Ok(())
}
