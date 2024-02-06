use ina219::address::Address;
use ina219::calibration::Calibration;
use ina219::measurements::{CurrentRegister, PowerRegister};
use ina219::SyncIna219;
use linux_embedded_hal::I2cdev;
use std::error::Error;

struct MyCalib;

impl MyCalib {
    const R_OHM: f32 = 0.001; // 1mOhm
    const CURRENT_LSB: f32 = 1.0; // 1A

    fn new() -> Self {
        Self
    }
}

impl Calibration for MyCalib {
    type Current = u16; // in A
    type Power = u16; // in W

    fn register_bits(&self) -> u16 {
        (0.04096 / (Self::CURRENT_LSB * Self::R_OHM)) as u16
    }

    fn current_from_register(&self, reg: CurrentRegister) -> Self::Current {
        reg.0
    }

    fn power_from_register(&self, reg: PowerRegister) -> Self::Power {
        reg.0 * 20
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let device = I2cdev::new("/dev/i2c-1")?;
    let mut ina = SyncIna219::new_calibrated(device, Address::from_byte(0x42)?, MyCalib::new())?;

    let measurements = ina.next_measurement()?.expect("Measurement is done");

    println!("{:#?}", measurements);

    // NOTE: The calibration can introduce quite a bit of error, we allow for 10% for testing

    let shunt = measurements.shunt_voltage.shunt_voltage_mv() as u16;
    let err = shunt / 10;
    let plausible_currents = (shunt - err)..(shunt + err);
    assert!(plausible_currents.contains(&measurements.current));

    let expected_power = measurements.current * (measurements.bus_voltage.voltage_mv() / 1000);
    let err = expected_power / 10;
    let plausible_power = (expected_power - err)..(expected_power + err);
    assert!(plausible_power.contains(&measurements.power));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ina219::calibration::simulate;
    use ina219::measurements::{BusVoltage, ShuntVoltage};

    #[test]
    fn does_not_overflow() {
        // Worst case is we get 16A at 20V over our 1mOhm resistor

        let bus = BusVoltage::from_mv(20_000); // 20V = 20_000mV
        let shunt = ShuntVoltage::from_10uv(16_000 / 10); // 0.001 Ohm * 16A = 0.016V = 16_000µV
        assert_eq!(shunt.shunt_voltage_uv(), 16_000);

        let measurements = simulate(&MyCalib, bus, shunt).expect("Does not overflow");

        assert!((15..17).contains(&measurements.current)); // Calculation does include some error
        assert_eq!(measurements.power, measurements.current * 20);
    }
}
