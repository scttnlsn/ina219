//! I2C address of the INA219 on the bus
//!
//! The address is set via the pins A0 and A1. The exact mapping can be seen in table 1 of the
//! datasheet. The address can be set either as a byte or as two [pins](Pin).

use core::fmt::Formatter;
use core::ops::RangeInclusive;

/// Names of the signal an address pin is connected to
///
/// The values match the bits as used for addressing the INA219. See table 1 of the datasheet for
/// reference.
///
/// # Example
/// ```rust
/// use ina219::address::Pin;
///
/// assert_eq!(Pin::Gnd.as_byte(), 0b00);
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Pin {
    /// The pin is connected to GND
    Gnd = 0,
    /// The pin is connected to Vcc
    Vcc = 1,
    /// The pin is connected to SDA
    Sda = 2,
    /// The pin is connected to SCL
    Scl = 3,
}

impl Pin {
    /// Get the value of the two lowest bits represented by connecting an address pin to this signal
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        self as u8
    }

    const fn from_lowest_bits(byte: u8) -> Self {
        match byte & 0b11 {
            0 => Self::Gnd,
            1 => Self::Vcc,
            2 => Self::Sda,
            3 => Self::Scl,
            _ => panic!("Masking of only the lowest bits guarantees that the values lie in 0..=3"),
        }
    }

    #[cfg(test)]
    const fn all_values() -> [Self; 4] {
        [Self::Gnd, Self::Vcc, Self::Sda, Self::Scl]
    }
}

/// I2C address of the INA219 on the bus
///
/// # Example
/// The address can either be set based on the used pins.
/// ```rust
/// use ina219::address::{Address, Pin};
///
/// let address = Address::from_pins(Pin::Sda, Pin::Scl);
/// assert_eq!(address.as_byte(), 0b100_1110);
/// ```
///
/// Or it can be set based on a byte. This will return `None` if the byte does not represent a valid address.
/// ```rust
/// use ina219::address::{Address, Pin};
///
/// let address = Address::from_byte(0b100_1011).unwrap();
/// assert_eq!(address.as_byte(), 0b100_1011);
///
/// assert!(Address::from_byte(42).is_none());
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Address {
    byte: u8,
}

impl Address {
    const VALID_ADDRESS: RangeInclusive<u8> = 0b100_0000..=0b100_1111;
    const MIN_ADDRESS: u8 = *Self::VALID_ADDRESS.start();
    const MAX_ADDRESS: u8 = *Self::VALID_ADDRESS.end();

    /// Create an address from the two pins A0 and A1
    ///
    /// # Example
    /// ```rust
    /// # use ina219::address::{Address, Pin};
    ///
    /// let address = Address::from_pins(Pin::Sda, Pin::Scl);
    /// assert_eq!(address.as_byte(), 0b100_1110);
    /// ```
    #[must_use]
    pub const fn from_pins(a0: Pin, a1: Pin) -> Self {
        let mut byte = 0b100_0000;

        byte |= a0.as_byte();
        byte |= a1.as_byte() << 2;

        Self { byte }
    }

    /// Create an address from a byte
    ///
    /// # Example
    /// ```rust
    /// # use ina219::address::{Address, Pin};
    ///
    /// let address = Address::from_byte(0b100_1011).unwrap();
    /// assert_eq!(address.as_byte(), 0b100_1011);
    /// ```
    ///
    /// # Errors
    /// This will return `Err` if the byte does not represent a valid address.
    /// ```rust
    /// # use ina219::address::{Address, Pin};
    ///
    /// assert!(Address::from_byte(42).is_none());
    /// ```
    pub const fn from_byte(byte: u8) -> Result<Self, OutOfRange> {
        match byte {
            Self::MIN_ADDRESS..=Self::MAX_ADDRESS => Ok(Self { byte }),
            which => Err(OutOfRange { which }),
        }
    }

    /// Get the address as a byte
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        self.byte
    }

    /// Get the address as two pins
    ///
    /// # Example
    /// ```rust
    /// # use ina219::address::{Address, Pin};
    ///
    /// let address = Address::from_byte(0b100_1011).unwrap();
    /// let (a0, a1) = address.as_pins();
    /// assert_eq!(a0, Pin::Scl);
    /// assert_eq!(a1, Pin::Sda);
    /// ```
    #[must_use]
    pub const fn as_pins(self) -> (Pin, Pin) {
        (
            Pin::from_lowest_bits(self.byte),
            Pin::from_lowest_bits(self.byte >> 2),
        )
    }
}

impl Default for Address {
    fn default() -> Self {
        Self::from_pins(Pin::Gnd, Pin::Gnd)
    }
}

/// The given address was not in the expected range for an INA219
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct OutOfRange {
    which: u8,
}

impl core::fmt::Display for OutOfRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "AddressOutOfRange: {:x}, should be in range: {:x}..={:x}",
            self.which,
            Address::MIN_ADDRESS,
            Address::MAX_ADDRESS,
        )
    }
}

impl TryFrom<u8> for Address {
    type Error = OutOfRange;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Address::from_byte(value)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OutOfRange {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_pin_reversible() {
        let mut bytes = vec![];

        for a0 in Pin::all_values() {
            for a1 in Pin::all_values() {
                let address = Address::from_pins(a0, a1);
                let (a0_, a1_) = address.as_pins();

                assert_eq!(a0, a0_);
                assert_eq!(a1, a1_);

                bytes.push(address.as_byte());
            }
        }

        bytes.sort_unstable();
        assert_eq!(bytes, (0b100_0000..=0b100_1111).collect::<Vec<u8>>());
    }

    #[test]
    fn is_byte_reversible() {
        for byte in 0b100_0000..=0b100_1111 {
            let address = Address::from_byte(byte).unwrap();
            let byte_ = address.as_byte();

            assert_eq!(byte, byte_);
        }
    }

    #[test]
    fn datasheet_examples() {
        use Pin::{Gnd, Scl, Sda, Vcc};

        let values = [
            // A1, A0, ADDRESS
            (Gnd, Gnd, 0b100_0000),
            (Gnd, Vcc, 0b100_0001),
            (Gnd, Sda, 0b100_0010),
            (Gnd, Scl, 0b100_0011),
            (Vcc, Gnd, 0b100_0100),
            (Vcc, Vcc, 0b100_0101),
            (Vcc, Sda, 0b100_0110),
            (Vcc, Scl, 0b100_0111),
            (Sda, Gnd, 0b100_1000),
            (Sda, Vcc, 0b100_1001),
            (Sda, Sda, 0b100_1010),
            (Sda, Scl, 0b100_1011),
            (Scl, Gnd, 0b100_1100),
            (Scl, Vcc, 0b100_1101),
            (Scl, Sda, 0b100_1110),
            (Scl, Scl, 0b100_1111),
        ];

        for (a1, a0, byte) in values.iter().copied() {
            let address = Address::from_pins(a0, a1);
            assert_eq!(address.as_byte(), byte);

            let (a0_, a1_) = Address::from_byte(byte).unwrap().as_pins();
            assert_eq!(a0, a0_);
            assert_eq!(a1, a1_);
        }
    }
}
