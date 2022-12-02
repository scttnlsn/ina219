#![no_std]

use embedded_hal::blocking::i2c;

pub mod configuration;

pub const INA219_ADDR: u8 = 0x41;

enum Register {
    Configuration = 0x00,
    ShuntVoltage = 0x01,
    BusVoltage = 0x02,
    Power = 0x03,
    Current = 0x04,
    Calibration = 0x05,
}

pub struct INA219<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C, E> INA219<I2C>
where
    I2C: i2c::Write<Error = E> + i2c::Read<Error = E>,
{
    pub fn new(i2c: I2C, address: u8) -> INA219<I2C> {
        INA219 { i2c, address }
    }

    pub fn configuration(&mut self) -> Result<configuration::Register, E> {
        let bits = self.read(Register::Configuration)?;
        Ok(configuration::Register::from_bits(bits))
    }

    pub fn set_configuratin(&mut self, conf: configuration::Register) -> Result<(), E> {
        self.write(Register::Configuration, conf.as_bits())
    }

    pub fn calibrate(&mut self, value: u16) -> Result<(), E> {
        self.write(Register::Calibration, value)
    }

    pub fn shunt_voltage(&mut self) -> Result<i16, E> {
        let value = self.read(Register::ShuntVoltage)?;
        Ok(value as i16)
    }

    pub fn voltage(&mut self) -> Result<u16, E> {
        let value = self.read(Register::BusVoltage)?;
        Ok((value >> 3) * 4)
    }

    pub fn power(&mut self) -> Result<i16, E> {
        let value = self.read(Register::Power)?;
        Ok(value as i16)
    }

    pub fn current(&mut self) -> Result<i16, E> {
        let value = self.read(Register::Current)?;
        Ok(value as i16)
    }

    fn read(&mut self, register: Register) -> Result<u16, E> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.i2c.write(self.address, &[register as u8])?;
        self.i2c.read(self.address, &mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn write(&mut self, register: Register, value: u16) -> Result<(), E> {
        let [val0, val1] = value.to_be_bytes();
        self.i2c.write(self.address, &[register as u8, val0, val1])
    }
}
