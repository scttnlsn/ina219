#![warn(clippy::pedantic)]
#![warn(clippy::missing_const_for_fn)]

use crate::calibration::Calibration;
use crate::configuration::ShuntVoltageRange;

/// A collection of all the measurements collected by the INA219
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct Measurements {
    pub bus_voltage: BusVoltage,
    pub shunt_voltage: ShuntVoltage,
    pub current: Current,
    pub power: Power,
}

/// A shunt voltage measurement as read from the shunt voltage register
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct ShuntVoltage(i16);

impl ShuntVoltage {
    /// Turns the bits read from the register into a `ShuntVoltage` checking that it is in the
    /// maximum value range of the INA219
    #[must_use]
    pub const fn from_bits(bits: u16) -> Option<Self> {
        Self::from_bits_with_range(bits, ShuntVoltageRange::Fsr320mv)
    }

    /// Turns the bits of the register into a `ShuntVoltage` checking that it is in the range given
    /// by `range`.
    #[must_use]
    pub const fn from_bits_with_range(bits: u16, range: ShuntVoltageRange) -> Option<Self> {
        let raw = Self::from_bits_unchecked(bits);
        let ten_uv = raw.shunt_voltage_10uv();
        let range = range.range_mv();
        if ten_uv >= *range.start() * 100 && ten_uv <= *range.end() * 100 {
            Some(raw)
        } else {
            None
        }
    }

    /// Turns the bits of the register into a `ShuntVoltage` without performing any range checks.
    #[must_use]
    pub const fn from_bits_unchecked(bits: u16) -> Self {
        Self(i16::from_ne_bytes(bits.to_ne_bytes()))
    }

    /// Get the shunt voltage in 10µV, this is the resolution used by the INA219.
    ///
    /// See also:
    /// * [`shunt_voltage_uv`] for measurement in µV
    /// * [`shunt_voltage_mv`] for measurement in mV
    #[must_use]
    pub const fn shunt_voltage_10uv(self) -> i16 {
        self.0
    }

    /// Get the shunt voltage in µV
    #[must_use]
    pub fn shunt_voltage_uv(self) -> i32 {
        i32::from(self.0) * 10
    }

    /// Get the shunt voltage in mV, truncating trailing digits
    #[must_use]
    pub const fn shunt_voltage_mv(self) -> i16 {
        self.0 / 100
    }
}

/// Contents of the bus voltage register
///
/// This contains next to the measurement also some flags about the last measurement.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct BusVoltage(u16);

impl BusVoltage {
    /// Create `BusVoltage` from the contents of the register.
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Return the bus voltage in the internal resolution of 4mV
    ///
    /// See also [`Self::voltage_mv`]
    #[must_use]
    pub const fn voltage_4mv(self) -> u16 {
        self.0 >> 3
    }

    /// Return the bus voltage in mV
    #[must_use]
    pub const fn voltage_mv(self) -> u16 {
        self.voltage_4mv() * 4
    }

    /// Check if the conversion ready flag is set
    ///
    /// The registers of the INA219 always return the last measurement value. But this flag can be
    /// used to check if **new** data is available.
    ///
    /// The flag is set when a conversion finished.
    /// The flag is cleared if:
    /// * The operation mode of the configuration register is written (except for [`PowerDone`] or [`AdcOff`])
    /// * The power register was read
    #[must_use]
    pub const fn is_conversion_ready(self) -> bool {
        self.0 & 0b10 != 0
    }

    /// This flag is set if the power or current calculation overflowed. Thus the power and/or
    /// current data might be wrong.
    #[must_use]
    pub const fn has_math_overflowed(self) -> bool {
        self.0 & 1 != 0
    }
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct Power {
    power: i16,
    power_lsb_uw: u32,
}

impl Power {
    /// Generate a `Power` reading from the register bits and the calibration that was used
    #[must_use]
    pub const fn from_bits_and_cal(bits: u16, calibration: Calibration) -> Self {
        Self {
            power: i16::from_ne_bytes(bits.to_ne_bytes()),
            power_lsb_uw: calibration.power_lsb_uw(),
        }
    }

    /// Try to get the measured current in µW as a `i32`
    ///
    /// Returns `None` if the calculation overflowed.
    #[must_use]
    pub fn try_current_ua_i32(self) -> Option<i32> {
        i32::checked_mul(
            i32::from(self.power),
            i32::try_from(self.power_lsb_uw).ok()?,
        )
    }

    /// Get the measured current in µA as an `i64` which can not overflow
    #[must_use]
    pub fn current_ua_i64(self) -> i64 {
        i64::from(self.power) * i64::from(self.power_lsb_uw)
    }
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct Current {
    current: i16,
    current_lsb_ua: u32,
}

impl Current {
    /// Generate a `Current` reading from the register bits and the calibration that was used
    #[must_use]
    pub const fn from_bits_and_cal(bits: u16, calibration: Calibration) -> Self {
        Self {
            current: i16::from_ne_bytes(bits.to_ne_bytes()),
            current_lsb_ua: calibration.current_lsb_ua(),
        }
    }

    /// Try to get the measured current in µA as a `i32`
    ///
    /// Returns `None` if the calculation overflowed.
    #[must_use]
    pub fn try_current_ua_i32(self) -> Option<i32> {
        i32::checked_mul(
            i32::from(self.current),
            i32::try_from(self.current_lsb_ua).ok()?,
        )
    }

    /// Get the measured current in µA as an `i64` which can not overflow
    #[must_use]
    pub fn current_ua_i64(self) -> i64 {
        i64::from(self.current) * i64::from(self.current_lsb_ua)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shunt_voltage() {
        // Samples from table 7 of the datasheet
        let targets = [
            (
                ShuntVoltage::from_bits(0).unwrap(),
                0, // 10µV
                0, // µV
                0, // mV
            ),
            (
                ShuntVoltage::from_bits(0b0111_1100_1111_1111).unwrap(),
                31999,   // 10µV
                319_990, // µV
                319,     // mV
            ),
            (
                ShuntVoltage::from_bits(0b1111_0000_0101_1111).unwrap(),
                -4001,  // 10µV
                -40010, // µV
                -40,    // mV
            ),
            (
                ShuntVoltage::from_bits(0b1000_0011_0000_0000).unwrap(),
                -32000,   // 10µV
                -320_000, // µV
                -320,     // mV
            ),
        ];
        for (sv, tuv, uv, mv) in targets {
            assert_eq!(sv.shunt_voltage_10uv(), tuv);
            assert_eq!(sv.shunt_voltage_uv(), uv);
            assert_eq!(sv.shunt_voltage_mv(), mv);
        }

        assert!(ShuntVoltage::from_bits(32001).is_none());
        assert!(ShuntVoltage::from_bits((-32001i16) as u16).is_none()); // -320.01 mV
    }

    #[test]
    fn bus_voltage() {
        let bv = BusVoltage::from_bits(0x1f40 << 3);
        assert_eq!(bv.voltage_mv(), 32_000);
        assert!(!bv.is_conversion_ready());
        assert!(!bv.has_math_overflowed());

        let bv = BusVoltage::from_bits(((0x1f40 / 2) << 3) | 0b11);
        assert_eq!(bv.voltage_mv(), 16_000);
        assert!(bv.is_conversion_ready());
        assert!(bv.has_math_overflowed());
    }

    #[test]
    fn current() {
        let calib = Calibration::new(1, 1_000_000).unwrap();

        let c = Current::from_bits_and_cal(0xFFFF, calib);
        assert_eq!(c.current_ua_i64(), -1);

        let c = Current::from_bits_and_cal(1, calib);
        assert_eq!(c.current_ua_i64(), 1);

        let calib = Calibration::new(u32::MAX, 1).unwrap();
        let c = Current::from_bits_and_cal(i16::MAX as u16, calib);
        assert_eq!(
            c.current_ua_i64(),
            i64::from(i16::MAX) * i64::from(u32::MAX)
        );
    }
}
