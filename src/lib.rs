#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![warn(clippy::pedantic)]
#![warn(clippy::missing_const_for_fn)]
#![warn(missing_docs)]

//! TODO: crate level docs

use crate::measurements::{CurrentRegister, Measurements, PowerRegister};
use configuration::{Configuration, Reset};
use core::fmt::{Debug, Display, Formatter};
use embedded_hal::blocking::i2c;
use measurements::{BusVoltage, ShuntVoltage};

pub mod address;
pub mod calibration;
pub mod configuration;
pub mod measurements;

pub use calibration::Calibration;

/// Addresses of the internal registers of the INA219
///
/// See [`INA219::read_raw()`]
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

impl Register {
    const fn name(self) -> &'static str {
        match self {
            Self::Configuration => "Configuration",
            Self::ShuntVoltage => "ShuntVoltage",
            Self::BusVoltage => "BusVoltage",
            Self::Power => "Power",
            Self::Current => "Current",
            Self::Calibration => "Calibration",
        }
    }
}

/// Error conditions that can appear during initialization
#[derive(Debug, Copy, Clone)]
pub enum InitializationError<I2cErr> {
    /// An I2C read or write failed
    I2cError(I2cErr),
    /// The configuration was not the default value after a reset
    ConfigurationNotDefaultAfterReset,
    /// A register was not zero when it was expected to be after reset
    RegisterNotZeroAfterReset(&'static str),
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

#[cfg(feature = "std")]
impl<I2cErr> std::error::Error for InitializationError<I2cErr>
where
    I2cErr: Debug + std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I2cError(err) => Some(err),
            Self::ConfigurationNotDefaultAfterReset
            | Self::BusVoltageOutOfRange
            | Self::RegisterNotZeroAfterReset(_)
            | Self::ShuntVoltageOutOfRange => None,
        }
    }
}

impl<I2cErr: Debug> Display for InitializationError<I2cErr> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(err) => write!(f, "I2C error: {err:?}"),
            Self::ConfigurationNotDefaultAfterReset => {
                write!(f, "Configuration was not default after reset")
            }
            Self::RegisterNotZeroAfterReset(reg) => {
                write!(f, "Register {reg:?} was not zero after reset")
            }
            Self::ShuntVoltageOutOfRange => write!(f, "Shunt voltage was out of range"),
            Self::BusVoltageOutOfRange => write!(f, "Bus voltage was out of range"),
        }
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
    /// The INA219 reported a math overflow for the given bus and shunt voltage
    MathOverflow(Measurements<(), ()>),
}

impl<E> From<E> for MeasurementError<E> {
    fn from(value: E) -> Self {
        Self::I2cError(value)
    }
}

#[cfg(feature = "std")]
impl<I2cErr> std::error::Error for MeasurementError<I2cErr>
where
    I2cErr: Debug + std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I2cError(err) => Some(err),
            Self::BusVoltageOutOfRange | Self::ShuntVoltageOutOfRange | Self::MathOverflow(_) => {
                None
            }
        }
    }
}

impl<I2cErr: Debug> Display for MeasurementError<I2cErr> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(err) => write!(f, "I2C error: {err:?}"),
            Self::ShuntVoltageOutOfRange => write!(f, "Shunt voltage was out of range"),
            Self::BusVoltageOutOfRange => write!(f, "Bus voltage was out of range"),
            Self::MathOverflow(Measurements {
                shunt_voltage,
                bus_voltage,
                ..
            }) => write!(
                f,
                "Math overflow for shunt voltage {shunt_voltage:?} and bus voltage {bus_voltage:?}"
            ),
        }
    }
}

/// Embedded HAL compatible driver for the INA219
pub struct INA219<I2C, Calib> {
    i2c: I2C,
    address: address::Address,
    config: Configuration,
    calib: Calib,
}

impl<I2C, E, Calib> INA219<I2C, Calib>
where
    I2C: i2c::Write<Error = E> + i2c::Read<Error = E>,
    Calib: Calibration,
{
    /// Open an INA219, perform a reset and check all register values are in the expected ranges
    ///
    /// # Errors
    /// If the device returns an unexpected response a `InitializationError` is returned.
    pub fn new(
        i2c: I2C,
        address: address::Address,
        calibration: Calib,
    ) -> Result<Self, InitializationError<E>> {
        let mut new = INA219::new_unchecked(i2c, address, Configuration::default(), calibration);

        new.reset()?;

        // We retry reading the configuration in case the device did not finish the reset yet
        let mut attempt = 0;
        loop {
            if new.configuration()? == Configuration::default() {
                break;
            }

            if attempt > 10 {
                return Err(InitializationError::ConfigurationNotDefaultAfterReset);
            }

            attempt += 1;
        }

        // Check that all calculated registers read zero after reset
        for reg in [Register::Calibration, Register::Current, Register::Power] {
            if new.read_raw(reg)? != 0 {
                return Err(InitializationError::RegisterNotZeroAfterReset(reg.name()));
            }
        }

        // Check that the shunt voltage is in range
        if ShuntVoltage::from_bits_with_range(
            new.read_raw(Register::ShuntVoltage)?,
            Configuration::default().shunt_voltage_range,
        )
        .is_none()
        {
            return Err(InitializationError::ShuntVoltageOutOfRange);
        }

        // Check that the bus voltage is in range
        if BusVoltage::from_bits_with_range(
            new.read_raw(Register::BusVoltage)?,
            Configuration::default().bus_voltage_range,
        )
        .is_none()
        {
            return Err(InitializationError::BusVoltageOutOfRange);
        }

        // Calibrate the device
        let bits = new.calib.register_bits();
        if bits != 0 {
            new.calibrate_raw(bits)?;
        }

        Ok(new)
    }

    /// Create a new `INA219` without performing a reset or checking registers for consistency
    pub const fn new_unchecked(
        i2c: I2C,
        address: address::Address,
        config: Configuration,
        calib: Calib,
    ) -> Self {
        INA219 {
            i2c,
            address,
            config,
            calib,
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
    pub fn calibrate(&mut self, value: Calib) -> Result<(), E> {
        self.calib = value;
        self.calibrate_raw(self.calib.register_bits())
    }

    fn calibrate_raw(&mut self, value: u16) -> Result<(), E> {
        self.write(Register::Calibration, value)
    }

    /// Checks if a new measurement was performed since the last configuration change,
    /// `Self::power` call or `Self::next_measurement`call returning Ok(None) if there is no new data
    ///
    /// TODO: Explain caveats around resetting the conversion ready flag
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when any of the
    /// measurements is outside of their expected ranges.
    #[allow(clippy::type_complexity)] // TODO:
                                      // Remove when https://github.com/rust-lang/rust/issues/8995 is resolved
    pub fn next_measurement(
        &mut self,
    ) -> Result<Option<Measurements<Calib::Current, Calib::Power>>, MeasurementError<E>> {
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
    pub fn power_raw(&mut self) -> Result<PowerRegister, E> {
        let bits = self.read_raw(Register::Power)?;
        Ok(PowerRegister(bits))
    }

    /// Read the last measured current
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub fn current_raw(&mut self) -> Result<CurrentRegister, E> {
        let bits = self.read_raw(Register::Current)?;
        Ok(CurrentRegister(bits))
    }

    /// Read the raw contents of a [`Register`]
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    fn read_raw(&mut self, register: Register) -> Result<u16, E> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.i2c.write(self.address.as_byte(), &[register as u8])?;
        self.i2c.read(self.address.as_byte(), &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn write(&mut self, register: Register, value: u16) -> Result<(), E> {
        let [val0, val1] = value.to_be_bytes();
        self.i2c
            .write(self.address.as_byte(), &[register as u8, val0, val1])
    }
}
