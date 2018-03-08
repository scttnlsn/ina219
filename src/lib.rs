extern crate byteorder;
extern crate i2cdev;

use byteorder::{ByteOrder, BigEndian};
use i2cdev::core::{I2CDevice};

pub const INA219_ADDR: u8 = 0x41;

enum Register {
    // Configuration = 0x00,
    ShuntVoltage = 0x01,
    BusVoltage = 0x02,
    Power = 0x03,
    Current = 0x04,
    Calibration = 0x05
}

pub struct INA219<T: I2CDevice> {
    device: T
}

impl<T> INA219<T> where T: I2CDevice {
    pub fn new(device: T) -> INA219<T> {
        INA219 { device: device }
    }

    pub fn calibrate(&mut self, values: &[u8]) -> Result<(), T::Error> {
        let mut data = vec![Register::Calibration as u8];
        data.extend(values.iter().cloned());
        self.device.write(&mut data)?;
        Ok(())
    }

    pub fn shunt_voltage(&mut self) -> Result<i16, T::Error> {
        let value = self.read(Register::ShuntVoltage)?;
        Ok(value as i16)
    }

    pub fn voltage(&mut self) -> Result<u16, T::Error> {
        let value = self.read(Register::BusVoltage)?;
        Ok((value >> 3) * 4)
    }

    pub fn power(&mut self) -> Result<i16, T::Error> {
        let value = self.read(Register::Power)?;
        Ok(value as i16)
    }

    pub fn current(&mut self) -> Result<i16, T::Error> {
        let value = self.read(Register::Current)?;
        Ok(value as i16)
    }

    fn read(&mut self, register: Register) -> Result<u16, T::Error> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.device.smbus_write_byte(register as u8)?;
        self.device.read(&mut buf)?;

        Ok(BigEndian::read_u16(&buf))
    }
}
