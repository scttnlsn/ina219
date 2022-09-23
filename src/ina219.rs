#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(unused_parens)]
#![allow(unused_must_use)]

extern crate byteorder;
extern crate embedded_hal as emb_hal;

use byteorder::{ByteOrder, BigEndian};
use emb_hal::blocking::i2c;

use std::fmt;

use crate::physic;

pub const INA219_ADDR: u8 = 0x42;

// Since physic electrical is in nano units we need to scale taking care to not
// overflow int64 or loose resolution.
const calibratescale: i64 = (((physic::Ampere as i64) * (physic::Ohm as i64)) / 100000) << 12;


pub struct Opts {
    Address: u8,
    SenseResistor: physic::ElectricResistance,
    MaxCurrent: physic::ElectricCurrent
}


impl Opts {
    pub fn new(add: u8, sense: physic::ElectricResistance,maxCurrent: physic::ElectricCurrent) -> Opts{
        Opts { 
            Address: add,
            SenseResistor: sense,
            MaxCurrent: maxCurrent,
             }
    }

    pub fn default() -> Opts {
        Opts { 
            Address: INA219_ADDR,
            SenseResistor: 100 * physic::MilliOhm, // 0.1Ohm
            MaxCurrent: 1 * physic::Ampere,
             }
    }

}

pub struct PowerMonitor {
    Shunt: String,
    Voltage: String,
    Current: String,
    Power: String,
}

impl PowerMonitor {
    pub fn new(shunt: String,
        voltage: String,
        current: String,
        power: String) -> PowerMonitor {
            PowerMonitor {
                Shunt: shunt,
                Voltage: voltage,
                Current: current,
                Power: power,
            }

    }
}

impl std::fmt::Display for PowerMonitor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Voltage = {},\nShunt_Voltage = {},\nCurrent = {},\nPower = {}",
        self.Voltage,
        self.Shunt,
        self.Current,
        self.Power
    )
    }
}

impl std::fmt::Debug for PowerMonitor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Debug: PowerMonitor \n{{\n\tVoltage = {},\n\tShunt_Voltage = {},\n\tCurrent = {},\n\tPower = {} \n}}",
        self.Voltage,
        self.Shunt,
        self.Current,
        self.Power
    )
    }
}


enum Register {
    Configuration = 0x00,
    ShuntVoltage = 0x01,
    BusVoltage = 0x02,
    Power = 0x03,
    Current = 0x04,
    Calibration = 0x05
}


trait ToStringPhysic_potential {
	fn to_string_physic_potential(self) -> String;
}

impl  ToStringPhysic_potential for  physic::ElectricPotential {
	fn to_string_physic_potential(self)  -> String {
		return physic::nanoAsString(self) + &"V";
	}
}

trait ToStringPhysic_power {
	fn to_string_physic_power(self) -> String;
}

impl ToStringPhysic_power for physic::Power {
	fn to_string_physic_power(self) -> String {
		return physic::nanoAsString(self) + &"W";
	}
}

trait ToStringPhysic_current {
	fn to_string_physic_current(self) -> String;
}

impl ToStringPhysic_current for  physic::ElectricCurrent {
	fn to_string_physic_current(self) -> String {
		return physic::nanoAsString(self) + &"A";
	}
}

trait ToStringPhysic_resistance {
	fn to_string_physic_resistance(self) -> String;
}

impl ToStringPhysic_resistance for  physic::ElectricResistance {
	fn to_string_physic_resistance(self) -> String {
		return physic::nanoAsString(self) + &"Ω";
	}
}


pub struct INA219<I2C> {
    i2c: I2C,
    opt: Opts,
    CurrentLSB: physic::ElectricCurrent,
    PowerLSB: physic::Power
}


impl<I2C,E> INA219<I2C>
    where I2C: i2c::Write<Error = E> + i2c::Read<Error = E>, E: std::fmt::Debug {
    pub fn new(i2c: I2C,opts: Opts) -> INA219<I2C> {
        INA219 {
            i2c,
            opt: opts,
            CurrentLSB: 0,
            PowerLSB: 0,
        }
    }

    pub fn init(&mut self) -> Result<(), E> {

        let ReConfigVal: u16 = 0x1FFF;

        self.calibrate(self.opt.SenseResistor,self.opt.MaxCurrent);

        //println!("self.opt.SenseResistor = {:?}",self.opt.SenseResistor.to_string_physic_resistance());
        //println!("self.opt.MaxCurrent = {:?}",self.opt.MaxCurrent.to_string_physic_current());
        self.i2c.write(self.opt.Address,
            &[Register::Configuration as u8,
            (ReConfigVal >> 8) as u8,
            ReConfigVal as u8
            ])?;
            Ok(())

    }

    pub fn calibrate(&mut self, sense: physic::ElectricResistance,maxCurrent: physic::ElectricCurrent) -> Result<(), E> {
        
        /*  to do
        if sense <= 0 {
            return Err(E::SenseResistorValueInvalid);
        }
        if maxCurrent <= 0 {
            return Err(E::MaxCurrentInvalid);
        }
        */
        self.CurrentLSB = maxCurrent / ( 1 << 15);
        self.PowerLSB = (((maxCurrent*20 + (1 << 14)) / (1 << 15)) as physic::Power);
        
    // Calibration Register = 0.04096 / (current LSB * Shunt Resistance)
	// Where lsb is in Amps and resistance is in ohms.
	// Calibration register is 16 bits.
        let cal = calibratescale / (( self.CurrentLSB as i64) * (sense as i64));
        //to do
        /* 
        if cal >= ( 1<< 16) {
            return Err();
        }
        */
        self.i2c.write(self.opt.Address, &[
            Register::Calibration as u8,
            (cal >> 8) as u8,
            cal as u8
        ])?;
        Ok(())
    }

    pub fn shunt_voltage(&mut self) -> Result<String, E> {
        //need to calibrate first
        self.calibrate(self.opt.SenseResistor, self.opt.MaxCurrent);

        let value = self.read(Register::ShuntVoltage)?;
        let str = value as i16;
        let tmp : String = (str as physic::ElectricPotential * physic::MicroVolt).to_string_physic_potential();
        Ok(tmp)
    }

    pub fn voltage(&mut self) -> Result<String, E> {
        //need to calibrate first
        self.calibrate(self.opt.SenseResistor, self.opt.MaxCurrent);

        let value = self.read(Register::BusVoltage)?;
        let str = ((value >> 3) *4) as i16;
        let tmp : String = (str as physic::ElectricPotential * physic::MilliVolt).to_string_physic_potential();
        Ok(tmp)
    }

    pub fn voltage_raw(&mut self) -> Result<u16, E> {
        //need to calibrate first
        self.calibrate(self.opt.SenseResistor, self.opt.MaxCurrent);
        let value = self.read(Register::BusVoltage)?;
        Ok((value >> 3) * 4)
    }

    pub fn power(&mut self) -> Result<String, E> {
        //need to calibrate first
        self.calibrate(self.opt.SenseResistor, self.opt.MaxCurrent);
        let value = self.read(Register::Power)?;
        let str = value as i16;
        let tmp : String = (str as physic::Power * physic::MilliVolt).to_string_physic_power();
        Ok(tmp)
    }

    pub fn current(&mut self) -> Result<String, E> {
        //need to calibrate first
        self.calibrate(self.opt.SenseResistor, self.opt.MaxCurrent);

        let value = self.read(Register::Current)?;
        //need to conver to i16 first
        let str = value as i16;
        //value  as physic::ElectricCurrent;
        let tmp : String = (str as physic::ElectricCurrent * physic::MilliAmpere).to_string_physic_current();
        Ok(tmp)
    }

    pub fn current_raw(&mut self) -> Result<i16, E> {
        //need to calibrate first
        self.calibrate(self.opt.SenseResistor, self.opt.MaxCurrent);

        let value = self.read(Register::Current)?;
        Ok(value as i16)
    }

    pub fn sense(&mut self) -> Result<PowerMonitor,E> {
        //need to calibrate first
        self.calibrate(self.opt.SenseResistor, self.opt.MaxCurrent);

        let shunt = self.shunt_voltage().unwrap();
        let voltage = self.voltage().unwrap();
        let current = self.current().unwrap();
        let power = self.power().unwrap();
        let pm = PowerMonitor::new(shunt,voltage,current,power);
        Ok(pm)


    }

    fn read(&mut self, register: Register) -> Result<u16, E> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.i2c.write(self.opt.Address, &[register as u8])?;
        self.i2c.read(self.opt.Address, &mut buf)?;
        Ok(BigEndian::read_u16(&buf))
    }
}



