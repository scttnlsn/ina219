use ina219::address::Address;
use ina219::configuration::{
    BusVoltageRange, Configuration, MeasuredSignals, OperatingMode, Reset, Resolution,
    ShuntVoltageRange,
};
use ina219::SyncIna219;
use linux_embedded_hal::I2cdev;
use std::error::Error;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    let device = I2cdev::new("/dev/i2c-1")?;
    let mut ina = SyncIna219::new(device, Address::from_byte(0x42)?)?;

    ina.set_configuration(Configuration {
        // Be extra precise, but take some extra time
        bus_resolution: Resolution::Avg128,
        shunt_resolution: Resolution::Avg128,

        // We only care about low voltage bus and shunt, values larger are truncated to the max
        bus_voltage_range: BusVoltageRange::Fsr16v,
        shunt_voltage_range: ShuntVoltageRange::Fsr40mv,

        // Measure both signals continuously (default)
        operating_mode: OperatingMode::Continous(MeasuredSignals::ShutAndBusVoltage),

        // Do not perform a reset
        reset: Reset::Run,
    })?;

    // Wait for the for measurement to be done
    let conversion_time: Duration = ina.configuration()?.conversion_time().unwrap();
    std::thread::sleep(conversion_time);

    let measurements = ina.next_measurement()?.expect("Conversion is done now");
    println!(
        "Bus:   {:.2}  V",
        measurements.bus_voltage.voltage_mv() as f32 / 1000.0
    );
    println!(
        "Shunt: {:.2} mV",
        measurements.shunt_voltage.shunt_voltage_mv() as f32
    );

    Ok(())
}
