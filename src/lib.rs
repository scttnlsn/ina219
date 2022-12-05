#![cfg_attr(not(test), no_std)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_const_for_fn)]
#![warn(missing_docs)]

//! TODO: crate level docs

use crate::measurements::{Current, MathErrors, Measurements, Power};
use configuration::{Configuration, Reset};
use embedded_hal::blocking::i2c;
use measurements::{BusVoltage, ShuntVoltage};

mod calibration;
pub mod configuration;
pub mod measurements;

pub use calibration::Calibration;

/// TODO: Replace with struct that handles pins
pub const INA219_ADDR: u8 = 0x41;

/// Addresses of the internal registers of the INA219
///
/// See [`INA219::read()`]
#[derive(Debug, Copy, Clone)]
pub enum Register {
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

/// Error conditions that can appear during initialization
#[derive(Debug, Copy, Clone)]
pub enum InitializationError<I2cErr> {
    /// An I2C read or write failed
    I2cError(I2cErr),
    /// The configuration was not the default value after a reset
    ConfigurationNotDefaultAfterReset,
    /// A register was not zero when it was expected to be after reset
    RegisterNotZeroAfterReset(Register),
    /// The shunt voltage value was not in the range expected after a reset
    ShuntVoltageOutOfRange,
    /// The bus voltage value was not in the range expected after a reset
    BusVoltageOutOfRange,
}

impl<E> From<E> for InitializationError<E> {
    fn from(value: E) -> Self {
        Self::I2cError(value)
    }
}

/// Errors that can happen when a measurement is read
#[derive(Debug, Copy, Clone)]
pub enum MeasurementError<I2cErr> {
    /// An I2C read or write failed
    I2cError(I2cErr),
    /// The shunt voltage was outside of the range given by the last set configuration
    ShuntVoltageOutOfRange,
    /// The bus voltage was outside of the range given by the last set configuration
    BusVoltageOutOfRange,
}

impl<E> From<E> for MeasurementError<E> {
    fn from(value: E) -> Self {
        Self::I2cError(value)
    }
}

/// Embedded HAL compatible driver for the INA219
pub struct INA219<I2C> {
    i2c: I2C,
    address: u8,
    config: Configuration,
    calib: Option<Calibration>,
}

impl<I2C, E> INA219<I2C>
where
    I2C: i2c::Write<Error = E> + i2c::Read<Error = E>,
{
    /// Open an INA219, perform a reset and check all register values are in the expected ranges
    ///
    /// # Errors
    /// If the device returns an unexpected response a `InitializationError` is returned.
    pub fn new(i2c: I2C, address: u8) -> Result<Self, InitializationError<E>> {
        let mut new = Self::new_unchecked(i2c, address, Configuration::default());

        new.reset()?;

        // TODO: Do we need to wait here?

        if new.configuration()? != Configuration::default() {
            return Err(InitializationError::ConfigurationNotDefaultAfterReset);
        }

        for reg in [Register::Calibration, Register::Current, Register::Power] {
            if new.read_raw(reg)? != 0 {
                return Err(InitializationError::RegisterNotZeroAfterReset(reg));
            }
        }

        if ShuntVoltage::from_bits_with_range(
            new.read_raw(Register::ShuntVoltage)?,
            Configuration::default().shunt_voltage_range,
        )
        .is_none()
        {
            return Err(InitializationError::ShuntVoltageOutOfRange);
        }

        if BusVoltage::from_bits_with_range(
            new.read_raw(Register::BusVoltage)?,
            Configuration::default().bus_voltage_range,
        )
        .is_none()
        {
            return Err(InitializationError::BusVoltageOutOfRange);
        }

        Ok(new)
    }

    /// Create a new `INA219` without performing a reset or checking registers for consistency
    pub const fn new_unchecked(i2c: I2C, address: u8, config: Configuration) -> INA219<I2C> {
        INA219 {
            i2c,
            address,
            config,
            calib: None,
        }
    }

    /// Perform a power-on-reset
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn reset(&mut self) -> Result<(), E> {
        self.set_configuration(Configuration {
            reset: Reset::Reset,
            ..Default::default()
        })
    }

    /// Read the current [`Configuration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn configuration(&mut self) -> Result<Configuration, E> {
        // TODO: How to handle case where read and self.config disagree

        let bits = self.read_raw(Register::Configuration)?;
        Ok(Configuration::from_bits(bits))
    }

    /// Set a new [`Configuration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn set_configuration(&mut self, conf: Configuration) -> Result<(), E> {
        match self.write(Register::Configuration, conf.as_bits()) {
            ok @ Ok(()) => {
                self.config = conf;
                ok
            }
            e @ Err(_) => {
                self.config = self.configuration()?;
                e
            }
        }
    }

    /// Set a new [`Calibration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub fn calibrate(&mut self, value: u16) -> Result<(), E> {
        self.write(Register::Calibration, value)
    }

    /// Checks if a new measurement was performed since the last configuration change,
    /// `Self::power` call or `Self::next_measurement`call returning Ok(None) if there is no new data
    ///
    /// TODO: Explain caveats around resetting the conversion ready flag
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when any of the
    /// measurements is outside of their expected ranges.
    pub fn next_measurement(&mut self) -> Result<Option<Measurements>, MeasurementError<E>> {
        let bus_voltage = self.bus_voltage()?;
        if !bus_voltage.is_conversion_ready() {
            // No new data... nothing to do...
            return Ok(None);
        }

        // Reset conversion ready flag
        let power = self.power_raw()?;

        let current_power = match (bus_voltage.has_math_overflowed(), self.calib) {
            (true, _) => Err(MathErrors::MathOverflow),
            (_, None) => Err(MathErrors::NoCalibration),
            (false, Some(calib)) => {
                let current = self.current_raw()?;
                Ok((
                    Current::from_bits_and_cal(current, calib),
                    Power::from_bits_and_cal(power, calib),
                ))
            }
        };

        Ok(Some(Measurements {
            bus_voltage,
            shunt_voltage: self.shunt_voltage()?,
            current_power,
        }))
    }

    /// Read the last measured shunt voltage
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when the shunt voltage
    /// is outside of the expected range given in the last written configuration.
    pub fn shunt_voltage(&mut self) -> Result<ShuntVoltage, MeasurementError<E>> {
        let value = self.read_raw(Register::ShuntVoltage)?;
        ShuntVoltage::from_bits_with_range(value, self.config.shunt_voltage_range)
            .ok_or(MeasurementError::ShuntVoltageOutOfRange)
    }

    /// Read the last measured bus voltage
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when the bus voltage
    /// is outside of the expected range given in the last written configuration.
    pub fn bus_voltage(&mut self) -> Result<BusVoltage, MeasurementError<E>> {
        let value = self.read_raw(Register::BusVoltage)?;
        BusVoltage::from_bits_with_range(value, self.config.bus_voltage_range)
            .ok_or(MeasurementError::BusVoltageOutOfRange)
    }

    /// Read the last measured power
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub fn power_raw(&mut self) -> Result<u16, E> {
        self.read_raw(Register::Power)
    }

    /// Read the last measured current
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub fn current_raw(&mut self) -> Result<u16, E> {
        self.read_raw(Register::Current)
    }

    /// Read the raw contents of a [`Register`]
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub fn read_raw(&mut self, register: Register) -> Result<u16, E> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.i2c.write(self.address, &[register as u8])?;
        self.i2c.read(self.address, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn write(&mut self, register: Register, value: u16) -> Result<(), E> {
        let [val0, val1] = value.to_be_bytes();
        self.i2c.write(self.address, &[register as u8, val0, val1])
    }
}
