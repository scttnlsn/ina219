#![allow(clippy::module_name_repetitions)]

/// Addresses of the internal registers of the INA219
///
/// See [`INA219::read_raw()`]
#[repr(u8)]
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Copy, Clone)]
pub enum RegisterName {
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

pub trait Register {
    const ADDRESS: u8;
}

pub trait ReadRegister: Register {
    fn from_bits(bits: u16) -> Self;
}

pub trait WriteRegister: Register {
    fn as_bits(&self) -> u16;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::UnCalibrated;
    use crate::configuration::Configuration;
    use crate::measurements::{
        BusVoltageRegister, CurrentRegister, PowerRegister, ShuntVoltageRegister,
    };

    #[test]
    fn register_names_match() {
        assert_eq!(RegisterName::Configuration as u8, Configuration::ADDRESS);
        assert_eq!(
            RegisterName::ShuntVoltage as u8,
            ShuntVoltageRegister::ADDRESS
        );
        assert_eq!(RegisterName::BusVoltage as u8, BusVoltageRegister::ADDRESS);
        assert_eq!(RegisterName::Power as u8, PowerRegister::ADDRESS);
        assert_eq!(RegisterName::Current as u8, CurrentRegister::ADDRESS);
        assert_eq!(RegisterName::Calibration as u8, UnCalibrated::ADDRESS);
    }
}
