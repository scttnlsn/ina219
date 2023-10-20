#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_const_for_fn)]
#![warn(missing_docs)]

//! TODO: crate level docs

use crate::calibration::{Calibration, UnCalibrated};
use crate::configuration::{BusVoltageRange, ShuntVoltageRange};
use crate::errors::InitializationErrorReason;
use crate::measurements::{CurrentRegister, Measurements, PowerRegister};
use configuration::{Configuration, Reset};
use core::fmt::Debug;
use embedded_hal::i2c::I2c;
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

#[cfg(test)]
mod tests;

/// Addresses of the internal registers of the INA219
///
/// See [`INA219::read_raw()`]
#[repr(u8)]
#[derive(Debug, Copy, Clone)]
enum Register {
    /// Configuration register, see [`Configuration`]
    Configuration = 0x00,
    /// Shunt voltage register, see [`ShuntVoltage`]
    ShuntVoltage = 0x01,
    /// Bus voltage register, see [`BusVoltage`]
    BusVoltage = 0x02,
    /// Power register, see [`Power`]
    Power = 0x03,
    /// Current register, see [`Current`]
    Current = 0x04,
    /// Calibration register, see [`Calibration`]
    Calibration = 0x05,
}

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
        const MAX_RESET_READ_RETRIES: u8 = 10;

        self.reset()?;

        // We retry reading the configuration in case the device did not finish the reset yet
        let mut attempt = 0;
        loop {
            if self.read_configuration()? == Configuration::default() {
                break;
            }

            if attempt > MAX_RESET_READ_RETRIES {
                return Err(InitializationErrorReason::ConfigurationNotDefaultAfterReset);
            }

            attempt += 1;
        }

        // If we are paranoid we perform extra checks to verify we talk to a real INA219
        #[cfg(feature = "paranoid")]
        {
            // Check that all calculated registers read zero after reset
            for reg in [Register::Calibration, Register::Current, Register::Power] {
                if self.read_raw(reg)? != 0 {
                    return Err(InitializationErrorReason::RegisterNotZeroAfterReset(
                        errors::RegisterName(reg),
                    ));
                }
            }

            // Check that the shunt voltage is in range
            if ShuntVoltage::from_bits_with_range(
                self.read_raw(Register::ShuntVoltage)?,
                Configuration::default().shunt_voltage_range,
            )
            .is_none()
            {
                return Err(InitializationErrorReason::ShuntVoltageOutOfRange);
            }

            // Check that the bus voltage is in range
            if BusVoltage::from_bits_with_range(
                self.read_raw(Register::BusVoltage)?,
                Configuration::default().bus_voltage_range,
            )
            .is_none()
            {
                return Err(InitializationErrorReason::BusVoltageOutOfRange);
            }
        }

        // Calibrate the device
        let bits = self.calib.register_bits();
        if bits == 0 {
            // Do nothing
            // We can skip writing a calibration of 0 since that is the reset value
        } else {
            self.calibrate_raw(bits)?;
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
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn reset(&mut self) -> Result<(), I2C::Error> {
        self.set_configuration(Configuration {
            reset: Reset::Reset,
            ..Default::default()
        })
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
        let read = self.read_configuration()?;

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

    /// Read the configuration without any checks, which is needed during initialization
    fn read_configuration(&mut self) -> Result<Configuration, I2C::Error> {
        let bits = self.read_raw(Register::Configuration)?;
        Ok(Configuration::from_bits(bits))
    }

    /// Set a new [`Configuration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn set_configuration(&mut self, conf: Configuration) -> Result<(), I2C::Error> {
        let result = self.write_raw(Register::Configuration, conf.as_bits());

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
        self.calibrate_raw(self.calib.register_bits())
    }

    fn calibrate_raw(&mut self, value: u16) -> Result<(), I2C::Error> {
        self.write_raw(Register::Calibration, value)
    }

    /// Checks if a new measurement was performed since the last configuration change,
    /// [`Self::power_raw`] call or [`Self::next_measurement`] call returning Ok(None) if there is no new data
    ///
    /// TODO: Explain caveats around resetting the conversion ready flag
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when any of the
    /// measurements is outside of their expected ranges.
    #[allow(clippy::type_complexity)] // TODO:
                                      // Remove when https://github.com/rust-lang/rust/issues/8995 is resolved
    pub fn next_measurement(
        &mut self,
    ) -> Result<Option<Measurements<Calib>>, MeasurementError<I2C::Error>> {
        let bus_voltage = self.bus_voltage()?;
        if !bus_voltage.is_conversion_ready() {
            // No new data... nothing to do...
            return Ok(None);
        }

        // Reset conversion ready flag
        let power = self.power_raw()?;

        let shunt_voltage = self.shunt_voltage()?;

        if bus_voltage.has_math_overflowed() {
            return Err(MeasurementError::MathOverflow(Measurements {
                bus_voltage,
                shunt_voltage,
                current: (),
                power: (),
            }));
        }

        let current = if Calib::READ_CURRENT {
            self.current_raw()?
        } else {
            CurrentRegister(0)
        };

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
        let value = self.read_raw(Register::ShuntVoltage)?;

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
        let value = self.read_raw(Register::BusVoltage)?;

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
        let bits = self.read_raw(Register::Power)?;
        Ok(PowerRegister(bits))
    }

    /// Read the last measured current
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub fn current_raw(&mut self) -> Result<CurrentRegister, I2C::Error> {
        let bits = self.read_raw(Register::Current)?;
        Ok(CurrentRegister(bits))
    }

    /// Read the raw contents of a [`Register`]
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    fn read_raw(&mut self, register: Register) -> Result<u16, I2C::Error> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.i2c
            .write_read(self.address.as_byte(), &[register as u8], &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    /// Writes the raw contents of a [`Register`]
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    fn write_raw(&mut self, register: Register, value: u16) -> Result<(), I2C::Error> {
        let [val0, val1] = value.to_be_bytes();
        self.i2c
            .write(self.address.as_byte(), &[register as u8, val0, val1])
    }
}
