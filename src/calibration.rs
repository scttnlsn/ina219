//! Types and trait to calibrate the INA219
//!
//! **Note:** Using the calibration with the INA219 can introduce some errors into the current and power measurements.
//! And it only saves two multiplications in software. So usage of this module should be well reasoned and errors be
//! accounted for.

use crate::errors::MeasurementError;
use crate::measurements::{BusVoltage, CurrentRegister, Measurements, PowerRegister, ShuntVoltage};
use crate::register::{ReadRegister, Register, WriteRegister};
use core::fmt::{Display, Formatter};
use core::ops::RangeInclusive;

/// Trait describing a calibration for the INA219
pub trait Calibration {
    /// Type of the current measurement
    type Current;

    /// Type of the power measurement
    type Power;

    /// Indicate whether the calibration needs the current measurement to be read
    ///
    /// If false `current_from_register` will always be called with `0`.
    const READ_CURRENT: bool = true;

    /// Return the value that should be written to the calibration register for this calibration
    fn register_bits(&self) -> u16;

    /// Return the current measurement from the given register value
    fn current_from_register(&self, reg: CurrentRegister) -> Self::Current;

    /// Return the power measurement from the given register value
    fn power_from_register(&self, reg: PowerRegister) -> Self::Power;
}

/// Simulate the calculation a real INA219 would produce
///
/// # Errors
/// Returns [`MeasurementError::MathOverflow`] if the calculation would overflow.
///
/// # Example
/// ```
/// use ina219::calibration::{IntCalibration, MicroAmpere, MicroWatt, simulate};
/// use ina219::measurements::{BusVoltage, ShuntVoltage};
///
/// let calib = IntCalibration::new(MicroAmpere(1_000), 1_000_000).unwrap(); // 1mA, 1Ohm
/// assert_eq!(calib.as_bits(), 40);
///
/// let bus = BusVoltage::from_mv(20_000); // 20V
/// let shunt = ShuntVoltage::from_10uv(4000); // 40mV
///
/// let measurement = simulate(&calib, bus, shunt).expect("Does not overflow");
///
/// assert_eq!(measurement.current, MicroAmpere(39_000)); // ~40mA; But the calibration introduces some error
/// assert_eq!(measurement.power, MicroWatt(780_000)); // 20V * 39mA = 780mW
/// ```
pub fn simulate<C: Calibration>(
    calib: &C,
    bus_voltage: BusVoltage,
    shunt_voltage: ShuntVoltage,
) -> Result<Measurements<C::Current, C::Power>, MeasurementError<core::convert::Infallible>> {
    const MAX: u32 = u16::MAX as u32;

    let calib_reg: u32 = calib.register_bits().into();
    let current = (u32::from(shunt_voltage.raw()) * calib_reg) / 4096;

    let power = (current * u32::from(bus_voltage.voltage_4mv())) / 5000;
    if current > MAX || power > MAX {
        let on_error_measurement = Measurements {
            bus_voltage,
            shunt_voltage,
            current: (),
            power: (),
        };
        return Err(MeasurementError::MathOverflow(on_error_measurement));
    }

    // Both casts have been checked above
    #[allow(clippy::cast_possible_truncation)]
    Ok(Measurements {
        bus_voltage,
        shunt_voltage,
        current: calib.current_from_register(CurrentRegister(current as u16)),
        power: calib.power_from_register(PowerRegister(power as u16)),
    })
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

impl<C> Register for C
where
    C: Calibration,
{
    const ADDRESS: u8 = 5;
}

impl<C> WriteRegister for C
where
    C: Calibration,
{
    fn as_bits(&self) -> u16 {
        C::register_bits(self)
    }
}

/// Empty calibration that does not perform any calibration
///
/// Use this if you don't want to use the current or power measurements of the INA219
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct UnCalibrated;

impl Calibration for UnCalibrated {
    type Current = ();
    type Power = ();

    const READ_CURRENT: bool = false;

    fn register_bits(&self) -> u16 {
        0
    }
    fn current_from_register(&self, _reg: CurrentRegister) -> Self::Current {}
    fn power_from_register(&self, _reg: PowerRegister) -> Self::Power {}
}

/// Scaling factor derived from datasheet and µ SI prefix: 0.04096 * (1/µ)^2
const SCALING_FACTOR: u64 = 40_960_000_000;
const RANGE: RangeInclusive<u64> = (SCALING_FACTOR / (u16::MAX as u64))..=(SCALING_FACTOR / 2);

/// Calibration used by the INA219 to turn the shunt voltage into current and power measurements
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)] // Just Int is a bit to short
pub struct IntCalibration {
    /// Value of the least significant bit of the current in µA
    current_lsb: MicroAmpere,

    /// Resistance of the shunt resistor in µOhm
    r_shunt_uohm: u32,
}

impl IntCalibration {
    /// Create a new calibration using the least significant bit (LSB) of the current register in µV
    /// and the value of the shunt resistor used in µOhm

    // TODO: Add nicer error
    // TODO: Handle error introduced during calculation...
    #[must_use]
    pub fn new(current_lsb: MicroAmpere, r_shunt_uohm: u32) -> Option<Self> {
        if current_lsb.0 < 0 {
            return None;
        }
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
            i64::try_from(SCALING_FACTOR / (u64::from(bits) * u64::from(r_shunt_uohm))).ok()?;

        Self::new(MicroAmpere(current_lsb), r_shunt_uohm)
    }

    /// Turn this calibration into the bits that can be written to the calibration register
    #[must_use]
    pub const fn as_bits(self) -> u16 {
        // TryFrom is not const so we have to check by hand
        #[allow(clippy::cast_sign_loss)]
        let cur = match self.current_lsb.0 {
            cur @ 0.. => cur as u64,
            _ => unreachable!(),
        };

        let cal = SCALING_FACTOR / (cur * self.r_shunt_uohm as u64);

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

/// A current measurement in µA
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct MicroAmpere(pub i64);

impl Display for MicroAmpere {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} µA", self.0)
    }
}

/// A power measurement in µW
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct MicroWatt(pub i64);

impl Display for MicroWatt {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} µW", self.0)
    }
}

impl Calibration for IntCalibration {
    type Current = MicroAmpere;
    type Power = MicroWatt;

    fn register_bits(&self) -> u16 {
        Self::as_bits(*self)
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

pub(crate) struct RawCalibration(pub u16);

impl Calibration for RawCalibration {
    type Current = u16;
    type Power = u16;

    fn register_bits(&self) -> u16 {
        self.0
    }

    fn current_from_register(&self, reg: CurrentRegister) -> Self::Current {
        reg.0
    }

    fn power_from_register(&self, reg: PowerRegister) -> Self::Power {
        reg.0
    }
}

impl ReadRegister for RawCalibration {
    fn from_bits(bits: u16) -> Self {
        Self(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::cast_precision_loss)] // This is only used in tests
    fn as_bits_datasheet(cal: IntCalibration) -> u16 {
        let micro = 1.0 / 1_000_000.0;
        let current_lsb = cal.current_lsb.0 as f64 * micro;
        let r_shunt = f64::from(cal.r_shunt_uohm) * micro;

        let cal = f64::trunc(0.04096 / (current_lsb * r_shunt));
        assert!(
            !(cal < 0.0 || cal > f64::from(u16::MAX)),
            "Calculation out of range"
        );

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            cal as u16 & !1 // According to Figure 27 of the datasheet the lowest bit is always 0
        }
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
