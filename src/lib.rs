#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_const_for_fn)]
#![warn(missing_docs)]

//! TODO: crate level docs

use crate::address::Address;
use crate::calibration::{Calibration, UnCalibrated};
use crate::configuration::{BusVoltageRange, ShuntVoltageRange};
use crate::errors::InitializationErrorReason;
use crate::measurements::{
    BusVoltageRegister, CurrentRegister, Measurements, PowerRegister, ShuntVoltageRegister,
};
use crate::register::WriteRegister;
use configuration::{Configuration, Reset};
use embedded_hal::i2c::{ErrorType, I2c, Operation};
use errors::{
    BusVoltageReadError, ConfigurationReadError, InitializationError, MeasurementError,
    ShuntVoltageReadError,
};
use measurements::{BusVoltage, ShuntVoltage};

pub mod address;
pub mod calibration;
pub mod configuration;
pub mod errors;
pub mod measurements;

mod register;

#[cfg(test)]
mod tests;

/// Embedded HAL compatible driver for the INA219
pub struct INA219<I2C, Calib> {
    i2c: I2C,
    address: address::Address,
    #[cfg(feature = "paranoid")]
    config: Option<Configuration>,
    calib: Calib,
}

impl<I2C> INA219<I2C, UnCalibrated>
where
    I2C: I2c,
{
    /// Open an INA219 without calibration
    ///
    /// Performs a reset and if the `paranoid` feature is active checks all register values are in
    /// the expected ranges.
    ///
    /// # Errors
    /// If the device returns an unexpected response a `InitializationError` is returned.
    pub fn new(i2c: I2C, address: address::Address) -> Result<Self, InitializationError<I2C>> {
        Self::new_calibrated(i2c, address, UnCalibrated)
    }
}

impl<I2C, Calib> INA219<I2C, Calib>
where
    I2C: I2c,
    Calib: Calibration,
{
    /// Open an INA219, perform a reset and check all register values are in the expected ranges than apply the provided calibration
    ///
    /// # Errors
    /// If the device returns an unexpected response a `InitializationError` is returned.
    pub fn new_calibrated(
        i2c: I2C,
        address: address::Address,
        calibration: Calib,
    ) -> Result<Self, InitializationError<I2C>> {
        let mut new = INA219::new_unchecked(i2c, address, calibration);

        // This is done in a function to make error handling easier...
        // since we want to return the device in case something goes wrong
        match new.init() {
            Ok(()) => Ok(new),
            Err(e) => Err(InitializationError::new(e, new.destroy())),
        }
    }

    /// Perform the following steps on this device to bring it into a known state
    /// - Perform a Reset
    /// - Wait for the Reset to finish, by polling 10 times for if it is already done (are we there yet?)
    /// - If paranoid: Check if all registers are in the expected ranges
    /// - Apply the register value from self.calib
    fn init(&mut self) -> Result<(), InitializationErrorReason<I2C::Error>> {
        self.reset()?;

        // If we are paranoid we perform extra checks to verify we talk to a real INA219
        #[cfg(feature = "paranoid")]
        {
            use calibration::RawCalibration;
            use register::RegisterName;

            // read_configuration before should have populated the config which can now be used to
            // validate bus and shunt voltages
            assert!(self.config.is_some());

            // Check that all calculated registers read zero after reset
            if !matches!(self.read()?, RawCalibration(0)) {
                return Err(InitializationErrorReason::RegisterNotZeroAfterReset(
                    RegisterName::Calibration,
                ));
            }

            if !matches!(self.read()?, CurrentRegister(0)) {
                return Err(InitializationErrorReason::RegisterNotZeroAfterReset(
                    RegisterName::Current,
                ));
            }

            if !matches!(self.read()?, PowerRegister(0)) {
                return Err(InitializationErrorReason::RegisterNotZeroAfterReset(
                    RegisterName::Power,
                ));
            }

            // Check that the shunt voltage is in range
            self.shunt_voltage()?;

            // Check that the bus voltage is in range
            self.bus_voltage()?;
        }

        // Calibrate the device
        let bits = self.calib.register_bits();
        if bits == 0 {
            // Do nothing
            // We can skip writing a calibration of 0 since that is the reset value
        } else {
            write(&mut self.i2c, self.address, &self.calib)?;
        }

        Ok(())
    }

    /// Create a new `INA219` assuming the device is already initialized to the given values.
    ///
    /// This also does not write the given configuration or calibration.
    pub const fn new_unchecked(i2c: I2C, address: address::Address, calib: Calib) -> Self {
        INA219 {
            i2c,
            address,
            #[cfg(feature = "paranoid")]
            config: None,
            calib,
        }
    }

    /// Destroy the driver returning the underlying I2C device
    ///
    /// This does leave the device in it's current state.
    pub fn destroy(self) -> I2C {
        self.i2c
    }

    /// Perform a power-on-reset
    ///
    /// Make sure to set calibration after this finishes so self.calib matches what the device is
    /// calibrated to
    fn reset(&mut self) -> Result<(), InitializationErrorReason<I2C::Error>> {
        const MAX_RESET_READ_RETRIES: u8 = 10;

        // Set the reset bit
        self.set_configuration(Configuration {
            reset: Reset::Reset,
            ..Default::default()
        })?;

        // Wait until the device reports that it is done
        let mut attempt = 0;
        loop {
            if self.read::<Configuration>()? == Configuration::default() {
                return Ok(());
            }

            if attempt > MAX_RESET_READ_RETRIES {
                return Err(InitializationErrorReason::ConfigurationNotDefaultAfterReset);
            }

            attempt += 1;
        }
    }

    /// Read the current [`Configuration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    ///
    /// *With feature `paranoid`*:
    ///
    /// If the read configuration does not match the last saved configuration an error is returned
    /// and the saved configuration is updated to the read configuration.
    pub fn configuration(&mut self) -> Result<Configuration, ConfigurationReadError<I2C::Error>> {
        let read = self.read()?;

        #[cfg(feature = "paranoid")]
        {
            let saved = *self.config.get_or_insert(read);
            if read != saved {
                self.config = Some(read);
                return Err(ConfigurationReadError::ConfigurationMismatch { read, saved });
            }
        }

        Ok(read)
    }

    /// Set a new [`Configuration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn set_configuration(&mut self, conf: Configuration) -> Result<(), I2C::Error> {
        let result = self.write(conf);

        // TODO what to do in case this causes a reset? Just panic?

        #[cfg(feature = "paranoid")]
        {
            self.config = match result {
                Ok(()) => Some(conf),
                // We don't know anything about the current conf
                Err(_) => None,
            };
        }

        #[cfg_attr(not(feature = "paranoid"), allow(clippy::let_and_return))]
        result
    }

    /// Set a new [`Calibration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn calibrate(&mut self, value: Calib) -> Result<(), I2C::Error> {
        self.calib = value;
        write(&mut self.i2c, self.address, &self.calib)
    }

    /// Checks if a new measurement was performed since the last configuration change,
    /// [`Self::power_raw`] call or [`Self::next_measurement`] call returning Ok(None) if there is no new data
    ///
    /// TODO: Explain caveats around resetting the conversion ready flag
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when any of the
    /// measurements is outside of their expected ranges.
    pub fn next_measurement(
        &mut self,
    ) -> Result<Option<Measurements<Calib>>, MeasurementError<I2C::Error>> {
        let (bus_voltage, power, shunt_voltage, current) = if Calib::READ_CURRENT {
            self.read4()?
        } else {
            let (bus_voltage, power, shunt_voltage) = self.read3()?;
            (bus_voltage, power, shunt_voltage, CurrentRegister(0))
        };

        let bus_voltage = self.bus_voltage_from_register(bus_voltage)?;
        if !bus_voltage.is_conversion_ready() {
            // No new data... nothing to do...
            return Ok(None);
        }

        let shunt_voltage = self.shunt_voltage_from_register(shunt_voltage)?;

        if bus_voltage.has_math_overflowed() {
            return Err(MeasurementError::MathOverflow(Measurements {
                bus_voltage,
                shunt_voltage,
                current: (),
                power: (),
            }));
        }

        Ok(Some(Measurements {
            bus_voltage,
            shunt_voltage,
            current: self.calib.current_from_register(current),
            power: self.calib.power_from_register(power),
        }))
    }

    /// Read the last measured shunt voltage
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when the shunt voltage
    /// is outside of the expected range given in the last written configuration.
    pub fn shunt_voltage(&mut self) -> Result<ShuntVoltage, ShuntVoltageReadError<I2C::Error>> {
        let value: ShuntVoltageRegister = self.read()?;

        self.shunt_voltage_from_register(value)
    }

    fn shunt_voltage_from_register(
        &mut self,
        value: ShuntVoltageRegister,
    ) -> Result<ShuntVoltage, ShuntVoltageReadError<I2C::Error>> {
        // If we are paranoid we look up what we last set for the full range
        #[cfg(feature = "paranoid")]
        let shunt_voltage_range = self
            .config
            .map_or(ShuntVoltageRange::Fsr320mv, |c| c.shunt_voltage_range);

        // If we are not paranoid we still check that it is in the maximum range
        #[cfg(not(feature = "paranoid"))]
        let shunt_voltage_range = ShuntVoltageRange::Fsr320mv;

        ShuntVoltage::from_bits_with_range(value, shunt_voltage_range).ok_or_else(|| {
            ShuntVoltageReadError::ShuntVoltageOutOfRange {
                should: shunt_voltage_range,
                is: ShuntVoltage::from_bits_unchecked(value),
            }
        })
    }

    /// Read the last measured bus voltage
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when the bus voltage
    /// is outside of the expected range given in the last written configuration.
    pub fn bus_voltage(&mut self) -> Result<BusVoltage, BusVoltageReadError<I2C::Error>> {
        let value = self.read()?;

        self.bus_voltage_from_register(value)
    }

    fn bus_voltage_from_register(
        &mut self,
        value: BusVoltageRegister,
    ) -> Result<BusVoltage, BusVoltageReadError<I2C::Error>> {
        // If we are paranoid we look up what we last set for the full range
        #[cfg(feature = "paranoid")]
        let bus_voltage_range = self
            .config
            .map_or(BusVoltageRange::Fsr32v, |c| c.bus_voltage_range);

        // If we are not paranoid we still check that it is in the maximum range
        #[cfg(not(feature = "paranoid"))]
        let bus_voltage_range = BusVoltageRange::Fsr32v;

        BusVoltage::from_bits_with_range(value, bus_voltage_range).ok_or_else(|| {
            BusVoltageReadError::BusVoltageOutOfRange {
                should: bus_voltage_range,
                is: BusVoltage::from_bits_unchecked(value),
            }
        })
    }

    /// Read the last measured power
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub fn power_raw(&mut self) -> Result<PowerRegister, I2C::Error> {
        self.read()
    }

    /// Read the last measured current
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub fn current_raw(&mut self) -> Result<CurrentRegister, I2C::Error> {
        self.read()
    }

    fn read<Reg: register::ReadRegister>(&mut self) -> Result<Reg, I2C::Error> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.i2c
            .write_read(self.address.as_byte(), &[Reg::ADDRESS], &mut buf)?;
        Ok(Reg::from_bits(u16::from_be_bytes(buf)))
    }

    fn read3<R0, R1, R2>(&mut self) -> Result<(R0, R1, R2), I2C::Error>
    where
        R0: register::ReadRegister,
        R1: register::ReadRegister,
        R2: register::ReadRegister,
    {
        let mut b0: [u8; 2] = [0x00; 2];
        let mut b1: [u8; 2] = [0x00; 2];
        let mut b2: [u8; 2] = [0x00; 2];

        let mut transactions = [
            Operation::Write(&[R0::ADDRESS]),
            Operation::Read(&mut b0),
            Operation::Write(&[R1::ADDRESS]),
            Operation::Read(&mut b1),
            Operation::Write(&[R2::ADDRESS]),
            Operation::Read(&mut b2),
        ];

        self.i2c
            .transaction(self.address.as_byte(), &mut transactions[..])?;

        Ok((
            R0::from_bits(u16::from_be_bytes(b0)),
            R1::from_bits(u16::from_be_bytes(b1)),
            R2::from_bits(u16::from_be_bytes(b2)),
        ))
    }

    fn read4<R0, R1, R2, R3>(&mut self) -> Result<(R0, R1, R2, R3), I2C::Error>
    where
        R0: register::ReadRegister,
        R1: register::ReadRegister,
        R2: register::ReadRegister,
        R3: register::ReadRegister,
    {
        let mut b0: [u8; 2] = [0x00; 2];
        let mut b1: [u8; 2] = [0x00; 2];
        let mut b2: [u8; 2] = [0x00; 2];
        let mut b3: [u8; 2] = [0x00; 2];

        let mut transactions = [
            Operation::Write(&[R0::ADDRESS]),
            Operation::Read(&mut b0),
            Operation::Write(&[R1::ADDRESS]),
            Operation::Read(&mut b1),
            Operation::Write(&[R2::ADDRESS]),
            Operation::Read(&mut b2),
            Operation::Write(&[R3::ADDRESS]),
            Operation::Read(&mut b3),
        ];

        self.i2c
            .transaction(self.address.as_byte(), &mut transactions[..])?;

        Ok((
            R0::from_bits(u16::from_be_bytes(b0)),
            R1::from_bits(u16::from_be_bytes(b1)),
            R2::from_bits(u16::from_be_bytes(b2)),
            R3::from_bits(u16::from_be_bytes(b3)),
        ))
    }

    /// Write the value contained in the register to the address dictated by its type
    fn write(&mut self, reg: impl WriteRegister + Copy) -> Result<(), I2C::Error> {
        write(&mut self.i2c, self.address, &reg)
    }
}

// Since I do not want restrict calibration to be Clone we need a way to call write without having
// to give out both &mut self and &self
fn write<I2C: I2c, Reg: WriteRegister>(
    dev: &mut I2C,
    addr: Address,
    value: &Reg,
) -> Result<(), I2C::Error> {
    let [val0, val1] = value.as_bits().to_be_bytes();
    dev.write(addr.as_byte(), &[Reg::ADDRESS, val0, val1])
}
