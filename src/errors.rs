//! Errors that can be returned by the different functions

use crate::configuration::{BusVoltageRange, Configuration, ShuntVoltageRange};
use crate::measurements::{BusVoltage, Measurements, ShuntVoltage};
use crate::Register;
use core::fmt::{Debug, Display, Formatter};

/// Error conditions that can appear during initialization
#[derive(Debug, Copy, Clone)]
pub enum InitializationError<I2cErr> {
    /// An I2C read or write failed
    I2cError(I2cErr),
    /// The configuration was not the default value after a reset
    ConfigurationNotDefaultAfterReset,

    /// A register was not zero when it was expected to be after reset
    RegisterNotZeroAfterReset(RegisterName),
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
    /// An error occurred while reading the shunt voltage
    ShuntVoltageReadError(ShuntVoltageReadError<I2cErr>),
    /// An error occurred while reading the bus voltage
    BusVoltageReadError(BusVoltageReadError<I2cErr>),
    /// The INA219 reported a math overflow for the given bus and shunt voltage
    MathOverflow(Measurements<(), ()>),
}

impl<E> From<E> for MeasurementError<E> {
    fn from(value: E) -> Self {
        Self::I2cError(value)
    }
}

impl<E> From<ShuntVoltageReadError<E>> for MeasurementError<E> {
    fn from(value: ShuntVoltageReadError<E>) -> Self {
        match value {
            ShuntVoltageReadError::I2cError(e) => Self::I2cError(e),
            e @ ShuntVoltageReadError::ShuntVoltageOutOfRange { .. } => {
                Self::ShuntVoltageReadError(e)
            }
        }
    }
}

impl<E> From<BusVoltageReadError<E>> for MeasurementError<E> {
    fn from(value: BusVoltageReadError<E>) -> Self {
        match value {
            BusVoltageReadError::I2cError(e) => Self::I2cError(e),
            e @ BusVoltageReadError::BusVoltageOutOfRange { .. } => Self::BusVoltageReadError(e),
        }
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
            Self::ShuntVoltageReadError(err) => Some(err),
            Self::BusVoltageReadError(err) => Some(err),
            Self::MathOverflow(_) => None,
        }
    }
}

impl<I2cErr: Debug> Display for MeasurementError<I2cErr> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(err) => write!(f, "I2C error: {err:?}"),
            Self::ShuntVoltageReadError(err) => write!(f, "Shunt voltage read error: {err:?}"),
            Self::BusVoltageReadError(err) => write!(f, "Bus voltage read error: {err:?}"),
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

/// Errors that can happen when the shunt voltage is read
#[derive(Debug, Copy, Clone)]
pub enum ShuntVoltageReadError<I2cErr> {
    /// THE I2C read failed
    I2cError(I2cErr),
    /// The shunt voltage was out of range for the current configuration
    ShuntVoltageOutOfRange {
        /// Currently configured shunt voltage range
        should: ShuntVoltageRange,
        /// The shunt voltage that was read
        is: ShuntVoltage,
    },
}

impl<E> From<E> for ShuntVoltageReadError<E> {
    fn from(value: E) -> Self {
        Self::I2cError(value)
    }
}

impl<E: Debug> Display for ShuntVoltageReadError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(err) => write!(f, "I2C error: {err:?}"),
            Self::ShuntVoltageOutOfRange { should, is } => write!(
                f,
                "Shunt voltage was out of range, should be {should:?} but was {is:?}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl<I2cErr> std::error::Error for ShuntVoltageReadError<I2cErr>
where
    I2cErr: Debug + std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I2cError(err) => Some(err),
            Self::ShuntVoltageOutOfRange { .. } => None,
        }
    }
}

/// Errors that can happen when the bus voltage is read
#[derive(Debug, Copy, Clone)]
pub enum BusVoltageReadError<I2cErr> {
    /// The I2C read failed
    I2cError(I2cErr),
    /// The bus voltage was out of range for the current configuration
    BusVoltageOutOfRange {
        /// Currently configured bus voltage range
        should: BusVoltageRange,
        /// The bus voltage that was read
        is: BusVoltage,
    },
}

impl<E> From<E> for BusVoltageReadError<E> {
    fn from(value: E) -> Self {
        Self::I2cError(value)
    }
}

impl<E: Debug> Display for BusVoltageReadError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(err) => write!(f, "I2C error: {err:?}"),
            Self::BusVoltageOutOfRange { should, is } => write!(
                f,
                "Bus voltage was out of range, should be {should:?} but was {is:?}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl<I2cErr> std::error::Error for BusVoltageReadError<I2cErr>
where
    I2cErr: Debug + std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I2cError(err) => Some(err),
            Self::BusVoltageOutOfRange { .. } => None,
        }
    }
}

/// Errors that can happen when the configuration is read
#[derive(Debug, Copy, Clone)]
pub enum ConfigurationReadError<I2cErr> {
    /// The I2C read failed
    I2cError(I2cErr),
    /// The read configuration did not match the saved configuration
    ConfigurationMismatch {
        /// Configuration read from the device
        read: Configuration,
        /// Configuration saved in the driver
        saved: Configuration,
    },
}

impl<E> From<E> for ConfigurationReadError<E> {
    fn from(value: E) -> Self {
        Self::I2cError(value)
    }
}

impl<E: Debug> Display for ConfigurationReadError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2cError(err) => write!(f, "I2C error: {err:?}"),
            Self::ConfigurationMismatch { read, saved } => write!(
                f,
                "Configuration read from device {read:?} did not match saved configuration {saved:?}",
            ),
        }
    }
}

#[cfg(feature = "std")]
impl<I2cErr> std::error::Error for ConfigurationReadError<I2cErr>
where
    I2cErr: Debug + std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I2cError(err) => Some(err),
            Self::ConfigurationMismatch { .. } => None,
        }
    }
}

/// The name of a register, used in errors
#[derive(Copy, Clone)]
pub struct RegisterName(pub(crate) Register);

impl Debug for RegisterName {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
