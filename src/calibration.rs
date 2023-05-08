use crate::measurements::{CurrentRegister, PowerRegister};
use core::ops::RangeInclusive;

pub trait Calibration {
    type Current;
    type Power;

    fn register_bits(&self) -> u16;
    fn current_from_register(&self, reg: CurrentRegister) -> Self::Current;
    fn power_from_register(&self, reg: PowerRegister) -> Self::Power;
}

impl<T: Calibration> Calibration for Option<T> {
    type Current = Option<T::Current>;
    type Power = Option<T::Power>;

    fn register_bits(&self) -> u16 {
        match self {
            None => 0,
            Some(cal) => cal.register_bits(),
        }
    }

    fn current_from_register(&self, reg: CurrentRegister) -> Self::Current {
        self.as_ref().map(|cal| cal.current_from_register(reg))
    }

    fn power_from_register(&self, reg: PowerRegister) -> Self::Power {
        self.as_ref().map(|cal| cal.power_from_register(reg))
    }
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct UnCalibrated;

impl Calibration for UnCalibrated {
    type Current = ();
    type Power = ();

    fn register_bits(&self) -> u16 {
        0
    }

    fn current_from_register(&self, _reg: CurrentRegister) -> Self::Current {
        ()
    }

    fn power_from_register(&self, _reg: PowerRegister) -> Self::Power {
        ()
    }
}

/// Scaling factor derived from datasheet and µ SI prefix: 0.04096 * (1/µ)^2
const SCALING_FACTOR: u64 = 40_960_000_000;
const RANGE: RangeInclusive<u64> = (SCALING_FACTOR / (u16::MAX as u64))..=(SCALING_FACTOR / 2);

/// Calibration used by the INA219 to turn the shunt voltage into current and power measurements
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct IntCalibration {
    /// Value of the least significant bit of the current in µA
    current_lsb: MicroAmpere,

    /// Resistance of the shunt resistor in µOhm
    r_shunt_uohm: u32,
}

impl IntCalibration {
    /// Create a new calibration using the least significant bit (LSB) of the current register in µV
    /// and the value of the shunt resistor used in µOhm
    #[must_use]
    pub fn new(current_lsb: MicroAmpere, r_shunt_uohm: u32) -> Option<Self> {
        let product = u64::try_from(current_lsb.0).ok()? * u64::from(r_shunt_uohm);

        if RANGE.contains(&product) {
            Some(Self {
                current_lsb,
                r_shunt_uohm,
            })
        } else {
            None
        }
    }

    /// Reconstruct the calibration from the value read from the calibration register
    #[must_use]
    pub fn from_bits(bits: u16, r_shunt_uohm: u32) -> Option<Self> {
        if bits == 0 || r_shunt_uohm == 0 {
            return None;
        }

        let current_lsb =
            u32::try_from(SCALING_FACTOR / (u64::from(bits) * u64::from(r_shunt_uohm))).ok()?;

        Some(Self {
            // TODO: check unwrap
            current_lsb: MicroAmpere(current_lsb.try_into().unwrap()),
            r_shunt_uohm,
        })
    }

    /// Turn this calibration into the bits that can be written to the calibration register
    #[must_use]
    pub const fn as_bits(self) -> u16 {
        let cal = SCALING_FACTOR / (self.current_lsb.0 as u64 * self.r_shunt_uohm as u64);

        // try_from is not const and we do the check manually
        #[allow(clippy::cast_possible_truncation)]
        if cal >= 2 && cal <= u16::MAX as u64 {
            // According to Figure 27 of the datasheet the lowest bit is always 0
            (cal as u16) & !1
        } else {
            // This should be enforced by new/from_bits
            unreachable!()
        }
    }

    /// The value of the least significant bit in the current register in µV
    #[must_use]
    pub const fn current_lsb(self) -> MicroAmpere {
        self.current_lsb
    }

    /// The value of the least significant bit in the power register in µW
    #[must_use]
    pub const fn power_lsb(self) -> MicroWatt {
        MicroWatt(20 * self.current_lsb.0)
    }

    /// The value of the shunt used in µOhm
    #[must_use]
    pub const fn r_shunt_uohm(self) -> u32 {
        self.r_shunt_uohm
    }
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct MicroAmpere(pub i64);

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct MicroWatt(pub i64);

impl Calibration for IntCalibration {
    type Current = MicroAmpere;
    type Power = MicroWatt;

    fn register_bits(&self) -> u16 {
        self.as_bits()
    }

    fn current_from_register(&self, reg: CurrentRegister) -> Self::Current {
        MicroAmpere(self.current_lsb().0 * i64_from_signed_register(reg.0))
    }

    fn power_from_register(&self, reg: PowerRegister) -> Self::Power {
        MicroWatt(self.power_lsb().0 * i64_from_signed_register(reg.0))
    }
}

fn i64_from_signed_register(bits: u16) -> i64 {
    let sixteen = i16::from_ne_bytes(bits.to_ne_bytes());
    i64::from(sixteen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_bits_datasheet(cal: IntCalibration) -> u16 {
        let micro = 1.0 / 1_000_000.0;
        let current_lsb = cal.current_lsb.0 as f64 * micro;
        let r_shunt = f64::from(cal.r_shunt_uohm) * micro;

        let cal = f64::trunc(0.04096 / (current_lsb * r_shunt)) as u16;
        cal & !1 // According to Figure 27 of the datasheet the lowest bit is always 0
    }

    #[test]
    fn calculation_fits_datasheet() {
        for i in 1..=1_000 {
            for r in 1..=1_000 {
                if let Some(cal) = IntCalibration::new(MicroAmpere(i), r) {
                    assert_eq!(as_bits_datasheet(cal), cal.as_bits());
                }
            }
        }
    }
}
