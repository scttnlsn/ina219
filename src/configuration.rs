//! Types used to set the configuration for the INA219
//!
//! [`Register`] combines all the needed values for a register write.
//!
//! # Example
//! The `..` completion can be used to set specific values to change. For example:
//! ```rust
//! use ina219::configuration::{Configuration, Resolution};
//! let conf = Configuration {
//!     bus_resolution: Resolution::Avg128,
//!     shunt_resolution: Resolution::Avg128,
//!     .. Default::default()
//! };
//! ```

use core::ops::{RangeInclusive, RangeToInclusive};

/// Perform a system reset or continue work as normal
///
/// If set to `Reset` all registers are set to their defaults. The flag is cleared after the reset
/// was performed. So this should always read as `Run`.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Reset {
    /// Continue normal operation
    #[default]
    Run = 0,
    /// Perform system reset
    Reset = 1,
}

impl Reset {
    const SHIFT: u8 = 15;
    const MASK: u16 = 1;

    #[must_use]
    const fn from_register(reg: u16) -> Self {
        match (reg >> Self::SHIFT) & Self::MASK {
            0 => Self::Run,
            1 => Self::Reset,
            _ => unreachable!(),
        }
    }

    #[must_use]
    const fn apply_to_reg(self, mut reg: u16) -> u16 {
        reg &= !(Self::MASK << Self::SHIFT);
        reg |= (self as u16) << Self::SHIFT;
        reg
    }
}

/// Measurement range for the bus voltage
#[derive(Default, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum BusVoltageRange {
    /// Maximum bus voltage of 16V
    Fsr16v = 0,
    /// Maximum bus voltage of 32V (still limited by 26V IC maximum)
    #[default]
    Fsr32v = 1,
}

impl BusVoltageRange {
    const SHIFT: u8 = 13;
    const MASK: u16 = 1;

    /// The voltage range in Volts
    #[must_use]
    pub const fn range_v(self) -> RangeToInclusive<u16> {
        match self {
            BusVoltageRange::Fsr16v => ..=16,
            BusVoltageRange::Fsr32v => ..=32,
        }
    }

    #[must_use]
    const fn from_register(reg: u16) -> Self {
        match (reg >> Self::SHIFT) & Self::MASK {
            0 => Self::Fsr16v,
            1 => Self::Fsr32v,
            _ => unreachable!(),
        }
    }

    #[must_use]
    const fn apply_to_reg(self, mut reg: u16) -> u16 {
        reg &= !(Self::MASK << Self::SHIFT);
        reg |= (self as u16) << Self::SHIFT;
        reg
    }
}

/// Shunt voltage measurement range
///
/// This sets the value for the [PGA](https://en.wikipedia.org/wiki/Programmable-gain_amplifier) and
/// thus the maximum shunt voltage that can be measured.
#[derive(Default, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum ShuntVoltageRange {
    /// Range of ±40mV, gain of 1
    Fsr40mv = 0,
    /// Range of ±80mV, gain of 1/2
    Fsr80mv = 1,
    /// Range of ±160mV, gain of 1/4
    Fsr160mv = 2,
    /// Range of ±320mV, gain of 1/8
    #[default]
    Fsr320mv = 3,
}

impl ShuntVoltageRange {
    const SHIFT: u8 = 11;
    const MASK: u16 = 0b11;

    /// Maximum range in mV for the shunt voltage measurement
    #[must_use]
    pub const fn range_mv(self) -> RangeInclusive<i16> {
        match self {
            ShuntVoltageRange::Fsr40mv => -40..=40,
            ShuntVoltageRange::Fsr80mv => -80..=80,
            ShuntVoltageRange::Fsr160mv => -160..=160,
            ShuntVoltageRange::Fsr320mv => -320..=320,
        }
    }

    #[must_use]
    const fn from_register(reg: u16) -> Self {
        match (reg >> Self::SHIFT) & Self::MASK {
            0 => Self::Fsr40mv,
            1 => Self::Fsr80mv,
            2 => Self::Fsr160mv,
            3 => Self::Fsr320mv,
            4..=u16::MAX => unreachable!(),
        }
    }

    #[must_use]
    const fn apply_to_reg(self, mut reg: u16) -> u16 {
        reg &= !(Self::MASK << Self::SHIFT);
        reg |= (self as u16) << Self::SHIFT;
        reg
    }
}

/// Resolution / Averaging mode for shunt or bus voltage
///
/// This sets resolution which is used when sampling the voltages.
#[derive(Default, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum Resolution {
    /// Single 9 bit sample
    Res9Bit = 0b0000,
    /// Single 10 bit sample
    Res10Bit = 0b0001,
    /// Single 11 bit sample
    Res11Bit = 0b0010,
    /// Single 12 bit sample
    #[default]
    Res12Bit = 0b0011,
    /// 2 averaged 12 bit samples
    Avg2 = 0b1001,
    /// 4 averaged 12 bit samples
    Avg4 = 0b1010,
    /// 8 averaged 12 bit samples
    Avg8 = 0b1011,
    /// 16 averaged 12 bit samples
    Avg16 = 0b1100,
    /// 32 averaged 12 bit samples
    Avg32 = 0b1101,
    /// 64 averaged 12 bit samples
    Avg64 = 0b1110,
    /// 128 averaged 12 bit samples
    Avg128 = 0b1111,
}

impl Resolution {
    const SHIFT_BUS: u8 = 7;
    const SHIFT_SHUNT: u8 = 3;
    const MASK: u16 = 0b1111;

    #[must_use]
    const fn from_register<const SHIFT: u8>(reg: u16) -> Self {
        match (reg >> SHIFT) & Self::MASK {
            0b0000 | 0b0100 => Self::Res9Bit,
            0b0001 | 0b0101 => Self::Res10Bit,
            0b0010 | 0b0110 => Self::Res11Bit,
            0b0011 | 0b0111 | 0b1000 => Self::Res12Bit,
            0b1001 => Self::Avg2,
            0b1010 => Self::Avg4,
            0b1011 => Self::Avg8,
            0b1100 => Self::Avg16,
            0b1101 => Self::Avg32,
            0b1110 => Self::Avg64,
            0b1111 => Self::Avg128,
            0x10..=u16::MAX => unreachable!(), // The mask makes sure we will never get these values
        }
    }

    #[must_use]
    const fn apply_to_reg<const SHIFT: u8>(self, mut reg: u16) -> u16 {
        reg &= !(Self::MASK << SHIFT);
        reg |= (self as u16) << SHIFT;
        reg
    }

    #[must_use]
    const fn from_bus_register(reg: u16) -> Self {
        Self::from_register::<{ Self::SHIFT_BUS }>(reg)
    }

    #[must_use]
    const fn apply_to_bus_reg(self, reg: u16) -> u16 {
        self.apply_to_reg::<{ Self::SHIFT_BUS }>(reg)
    }

    #[must_use]
    const fn from_shunt_register(reg: u16) -> Self {
        Self::from_register::<{ Self::SHIFT_SHUNT }>(reg)
    }

    #[must_use]
    const fn apply_to_shunt_reg(self, reg: u16) -> u16 {
        self.apply_to_reg::<{ Self::SHIFT_SHUNT }>(reg)
    }

    /// Conversion time in µs when this resolution is active
    ///
    /// Values according to Table 5 in the datasheet.
    #[must_use]
    pub const fn conversion_time_us(self) -> u32 {
        match self {
            Resolution::Res9Bit => 84,
            Resolution::Res10Bit => 148,
            Resolution::Res11Bit => 276,
            Resolution::Res12Bit => 532,
            Resolution::Avg2 => 1_060,
            Resolution::Avg4 => 2_130,
            Resolution::Avg8 => 4_260,
            Resolution::Avg16 => 8_510,
            Resolution::Avg32 => 17_020,
            Resolution::Avg64 => 34_050,
            Resolution::Avg128 => 68_100,
        }
    }
}

/// Which signals are measured during a conversion
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum MeasuredSignals {
    /// Only the shunt voltage is measured
    ShuntVoltage = 1,
    /// Only the bus voltage is measured
    BusVoltage = 2,
    /// Both voltages are measured
    #[default]
    ShutAndBusVoltage = 3,
}

impl MeasuredSignals {
    #[must_use]
    const fn from_bits_wrapping(bits: u16) -> Self {
        match bits & 0b11 {
            0 => panic!(
                "Got passed 0 for signals to measure which should be cought be previous check!"
            ),
            1 => Self::ShuntVoltage,
            2 => Self::BusVoltage,
            3 => Self::ShutAndBusVoltage,
            4..=u16::MAX => unreachable!(), // The mask removes all other bits
        }
    }
}

/// Operation mode of the INA219
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum OperatingMode {
    /// Reduce power usage and disable current into the input pins
    ///
    /// Recovery takes 40µs.
    PowerDown = 0,
    /// Stop the conversions
    AdcOff = 0b100,
    /// Trigger a single conversion of the given signals
    Triggered(MeasuredSignals),
    /// Continuously measure the given signals
    Continous(MeasuredSignals),
}

impl OperatingMode {
    const SHIFT: u8 = 0;
    const MASK: u16 = 0b111;

    #[must_use]
    const fn from_register(reg: u16) -> Self {
        match (reg >> Self::SHIFT) & Self::MASK {
            0 => Self::PowerDown,
            0b100 => Self::AdcOff,
            x @ 1..=3 => Self::Triggered(MeasuredSignals::from_bits_wrapping(x)),
            x @ 5..=7 => Self::Continous(MeasuredSignals::from_bits_wrapping(x)),
            0b1000..=u16::MAX => unreachable!(),
        }
    }

    #[must_use]
    const fn apply_to_reg(self, mut reg: u16) -> u16 {
        reg &= !(Self::MASK << Self::SHIFT);
        reg |= (self.as_bits()) << Self::SHIFT;
        reg
    }

    /// Return the bits representing this mode
    #[must_use]
    pub const fn as_bits(self) -> u16 {
        match self {
            OperatingMode::PowerDown => 0,
            OperatingMode::AdcOff => 0b100,
            OperatingMode::Triggered(signals) => signals as u16,
            OperatingMode::Continous(signals) => signals as u16 | 0b100,
        }
    }
}

impl Default for OperatingMode {
    fn default() -> Self {
        OperatingMode::Continous(MeasuredSignals::ShutAndBusVoltage)
    }
}

/// Configuration register
///
/// Configures the way the INA219 performs its measurements.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct Configuration {
    /// Indicate to perform a reset or continue to run normally
    pub reset: Reset,
    /// Maximum measurement range for the bus voltage
    pub bus_voltage_range: BusVoltageRange,
    /// Maximum measurement range for the shunt voltage
    pub shunt_voltage_range: ShuntVoltageRange,
    /// Resolution / Averaging mode for the bus voltage measurement
    pub bus_resolution: Resolution,
    /// Resolution / Averaging mode for the shunt voltage measurement
    pub shunt_resolution: Resolution,
    /// Which signals to measure and if continous or triggered operation is set up
    pub operating_mode: OperatingMode,
}

impl Configuration {
    /// Turn the bits describing the configuration into a `Register`
    #[must_use]
    pub const fn from_bits(reg: u16) -> Self {
        let reset = Reset::from_register(reg);
        let operating_mode = OperatingMode::from_register(reg);
        let shunt_resolution = Resolution::from_shunt_register(reg);
        let bus_resolution = Resolution::from_bus_register(reg);
        let shunt_voltage_range = ShuntVoltageRange::from_register(reg);
        let bus_voltage_range = BusVoltageRange::from_register(reg);

        Self {
            reset,
            bus_voltage_range,
            shunt_voltage_range,
            bus_resolution,
            shunt_resolution,
            operating_mode,
        }
    }

    /// Turn this `Register` into the bits it describes
    #[must_use]
    pub const fn as_bits(self) -> u16 {
        let Self {
            reset,
            bus_voltage_range,
            shunt_voltage_range,
            bus_resolution,
            shunt_resolution,
            operating_mode,
        } = self;

        let mut bits = 0;
        bits = reset.apply_to_reg(bits);
        bits = bus_voltage_range.apply_to_reg(bits);
        bits = shunt_voltage_range.apply_to_reg(bits);
        bits = bus_resolution.apply_to_bus_reg(bits);
        bits = shunt_resolution.apply_to_shunt_reg(bits);
        bits = operating_mode.apply_to_reg(bits);
        bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_datasheet() {
        let reset_value = 0b0011_1001_1001_1111;

        assert_eq!(Configuration::default().as_bits(), reset_value);
        assert_eq!(
            Configuration::from_bits(reset_value),
            Configuration::default()
        );
    }

    #[test]
    fn is_inverse() {
        // We can not directly check if the same bit pattern is created because some patterns (like
        // 12Bit resolution) have redundant representations...
        // So we first turn the bits into a full description and then test if we can invert it

        // Interestingly it is quite fast to check all 2^16 patterns...
        for val in 0..=u16::MAX {
            let register = Configuration::from_bits(val);
            let bits_cleaned = register.as_bits();
            assert_eq!(register, Configuration::from_bits(bits_cleaned));

            if register.shunt_resolution != Resolution::Res12Bit
                && register.bus_resolution != Resolution::Res12Bit
            {
                // Ignore both *ADC3 bits and the unused bits as they are (sometimes) don't care
                let bits_to_ignore = 0b0100_0010_0010_0000;

                assert_eq!(val | bits_to_ignore, bits_cleaned | bits_to_ignore);
            }
        }
    }
}
