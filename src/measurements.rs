//! Types wrapping the measurements of the INA219
//!
//! These types help converting the ras register values into expressive values.
use crate::calibration::Calibration;
use crate::configuration::{BusVoltageRange, ShuntVoltageRange};

#[cfg(doc)]
use crate::configuration::OperatingMode::{AdcOff, PowerDown};

/// A collection of all the measurements collected by the INA219
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Measurements<Calib: Calibration> {
    /// Measured `BusVoltage`
    pub bus_voltage: BusVoltage,
    /// Measured `ShuntVoltage`
    pub shunt_voltage: ShuntVoltage,
    /// Measured `Current`
    pub current: Calib::Current,
    /// Measured `Power`
    pub power: Calib::Power,
}

/// Errors that can arise when current and power are calculated
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MathErrors {
    /// The INA219 reported a math overflow during the calculation
    MathOverflow,
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

    /// Get the shunt voltage in 10µV, this is the resolution reported by the INA219.
    ///
    /// See also:
    /// * [`Self::shunt_voltage_uv`] for measurement in µV
    /// * [`Self::shunt_voltage_mv`] for measurement in mV
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
    /// Create `BusVoltage` from the contents of the register checking that it is `range`.
    #[must_use]
    pub const fn from_bits_with_range(bits: u16, range: BusVoltageRange) -> Option<Self> {
        let new = Self(bits);

        if new.voltage_mv() <= (range.range_v().end * 1000) {
            Some(new)
        } else {
            None
        }
    }

    /// Create `BusVoltage` from the contents of the register. Performing no range checks.
    #[must_use]
    pub const fn from_bits_unchecked(bits: u16) -> Self {
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
    /// * The operation mode of the configuration register is written (except for [`PowerDown`] or [`AdcOff`])
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

/// The raw value read from the current register
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct CurrentRegister(pub u16);

/// The raw value read from the power register
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct PowerRegister(pub u16);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::{Calibration, IntCalibration, MicroAmpere};

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

        // -320.01 mV
        assert!(ShuntVoltage::from_bits(u16::from_ne_bytes(i16::to_ne_bytes(-32001))).is_none());
    }

    #[test]
    fn bus_voltage() {
        let bv = BusVoltage::from_bits_unchecked(0x1f40 << 3);
        assert_eq!(bv.voltage_mv(), 32_000);
        assert!(!bv.is_conversion_ready());
        assert!(!bv.has_math_overflowed());

        let bv = BusVoltage::from_bits_unchecked(((0x1f40 / 2) << 3) | 0b11);
        assert_eq!(bv.voltage_mv(), 16_000);
        assert!(bv.is_conversion_ready());
        assert!(bv.has_math_overflowed());
    }

    #[test]
    fn current() {
        let calib = IntCalibration::new(MicroAmpere(1), 1_000_000).unwrap();

        let c = calib.current_from_register(CurrentRegister(0xFFFF));
        assert_eq!(c.0, -1);

        let c = calib.current_from_register(CurrentRegister(1));
        assert_eq!(c.0, 1);

        let calib = IntCalibration::new(MicroAmpere(i64::from(u32::MAX)), 1).unwrap();
        let c = calib.current_from_register(CurrentRegister(i16::MAX as u16));
        assert_eq!(c.0, i64::from(i16::MAX) * i64::from(u32::MAX));
    }
}
