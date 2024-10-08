#![cfg_attr(not(any(feature = "sync", feature = "async")), allow(dead_code))]

//! Types wrapping the measurements of the INA219
//!
//! These types help converting the ras register values into expressive values.
use crate::configuration::{BusVoltageRange, ShuntVoltageRange};
use core::fmt::{Debug, Display, Formatter};

#[cfg(doc)]
use crate::configuration::OperatingMode::{AdcOff, PowerDown};
use crate::register::{ReadRegister, Register};

/// A collection of all the measurements collected by the INA219
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Measurements<Current, Power> {
    /// Measured `BusVoltage`
    pub bus_voltage: BusVoltage,
    /// Measured `ShuntVoltage`
    pub shunt_voltage: ShuntVoltage,
    /// Measured `Current`
    pub current: Current,
    /// Measured `Power`
    pub power: Power,
}

/// Errors that can arise when current and power are calculated
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MathErrors {
    /// The INA219 reported a math overflow during the calculation
    MathOverflow,
}

/// A shunt voltage measurement as read from the shunt voltage register
#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub struct ShuntVoltage(i16);

impl ShuntVoltage {
    /// Turns the bits read from the register into a `ShuntVoltage` checking that it is in the
    /// maximum value range of the INA219
    #[cfg(test)]
    const fn from_bits(bits: u16) -> Option<Self> {
        Self::from_bits_with_range(ShuntVoltageRegister(bits), ShuntVoltageRange::Fsr320mv)
    }

    /// Turns the bits of the register into a `ShuntVoltage` checking that it is in the range given
    /// by `range`.
    #[must_use]
    pub(crate) const fn from_bits_with_range(
        reg: ShuntVoltageRegister,
        range: ShuntVoltageRange,
    ) -> Option<Self> {
        let raw = Self::from_bits_unchecked(reg);
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
    pub(crate) const fn from_bits_unchecked(reg: ShuntVoltageRegister) -> Self {
        Self(i16::from_ne_bytes(reg.0.to_ne_bytes()))
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

    /// For testing: create a `ShuntVoltage` from a value of unit 10µV
    ///
    /// # Example
    /// ```
    /// use ina219::measurements::ShuntVoltage;
    /// assert_eq!(ShuntVoltage::from_10uv(100).shunt_voltage_uv(), 1_000);
    /// ```
    #[must_use]
    pub const fn from_10uv(uv: i16) -> Self {
        Self(uv)
    }

    pub(crate) const fn raw(self) -> u16 {
        u16::from_ne_bytes(self.0.to_ne_bytes())
    }
}

impl Display for ShuntVoltage {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} µV", self.shunt_voltage_uv())
    }
}

impl Debug for ShuntVoltage {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShuntVoltage")
            .field("micro_volt", &self.shunt_voltage_uv())
            .finish()
    }
}

#[derive(Copy, Clone)]
pub(crate) struct ShuntVoltageRegister(u16);

impl Register for ShuntVoltageRegister {
    const ADDRESS: u8 = 1;
}

impl ReadRegister for ShuntVoltageRegister {
    fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
}

/// Contents of the bus voltage register
///
/// This contains next to the measurement also some flags about the last measurement.
#[derive(Default, Copy, Clone, Eq, PartialEq)]
pub struct BusVoltage(u16);

impl BusVoltage {
    /// Create `BusVoltage` from the contents of the register checking that it is `range`.
    #[must_use]
    pub(crate) const fn from_bits_with_range(
        reg: BusVoltageRegister,
        range: BusVoltageRange,
    ) -> Option<Self> {
        let new = Self(reg.0);

        if new.voltage_mv() <= (range.range_v().end * 1000) {
            Some(new)
        } else {
            None
        }
    }

    /// Create `BusVoltage` from the contents of the register. Performing no range checks.
    #[must_use]
    pub(crate) const fn from_bits_unchecked(reg: BusVoltageRegister) -> Self {
        Self(reg.0)
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

    /// For testing: Create a `BusVoltage` from a given value in mV
    ///
    /// The overflow flag, and the ready flag will both be false.
    #[must_use]
    pub const fn from_mv(mv: u16) -> Self {
        Self((mv / 4) << 3)
    }
}

impl Display for BusVoltage {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} mV", self.voltage_mv())
    }
}

impl Debug for BusVoltage {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BusVoltage")
            .field("milli_volt", &self.voltage_mv())
            .field("has_math_overflowed", &self.has_math_overflowed())
            .field("is_conversion_ready", &self.is_conversion_ready())
            .finish()
    }
}

#[derive(Copy, Clone)]
pub(crate) struct BusVoltageRegister(u16);

impl Register for BusVoltageRegister {
    const ADDRESS: u8 = 2;
}

impl ReadRegister for BusVoltageRegister {
    fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
}

/// The raw value read from the current register
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct CurrentRegister(pub u16);

impl Register for CurrentRegister {
    const ADDRESS: u8 = 4;
}

impl ReadRegister for CurrentRegister {
    fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
}

/// The raw value read from the power register
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct PowerRegister(pub u16);

impl Register for PowerRegister {
    const ADDRESS: u8 = 3;
}

impl ReadRegister for PowerRegister {
    fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
}

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
    fn shunt_from_value() {
        for x in [i16::MIN, -2, 1, 0, 1, 2, 42, i16::MAX] {
            let mul_10 = x / 10;
            assert_eq!(
                ShuntVoltage::from_10uv(mul_10).shunt_voltage_uv(),
                i32::from(mul_10) * 10
            );
        }
    }

    #[test]
    fn bus_voltage() {
        let bv = BusVoltage::from_bits_unchecked(BusVoltageRegister(0x1f40 << 3));
        assert_eq!(bv.voltage_mv(), 32_000);
        assert!(!bv.is_conversion_ready());
        assert!(!bv.has_math_overflowed());

        let bv = BusVoltage::from_bits_unchecked(BusVoltageRegister(((0x1f40 / 2) << 3) | 0b11));
        assert_eq!(bv.voltage_mv(), 16_000);
        assert!(bv.is_conversion_ready());
        assert!(bv.has_math_overflowed());
    }

    #[test]
    fn bus_from_value() {
        for x in [0, 4, 8, 42, 32_000] {
            let mul_4 = x & !0b11;
            assert_eq!(BusVoltage::from_mv(mul_4).voltage_mv(), mul_4);
        }
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
