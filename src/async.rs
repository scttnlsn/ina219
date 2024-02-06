use crate::address::Address;
use crate::calibration::{Calibration, UnCalibrated};
use crate::configuration::{BusVoltageRange, Configuration, Reset, ShuntVoltageRange};
use crate::errors::{
    BusVoltageReadError, ConfigurationReadError, InitializationError, InitializationErrorReason,
    MeasurementError, ShuntVoltageReadError,
};
use crate::measurements::{
    BusVoltage, BusVoltageRegister, CurrentRegister, Measurements, PowerRegister, ShuntVoltage,
    ShuntVoltageRegister,
};
use crate::register::WriteRegister;
use crate::{address, register};
use embedded_hal_async::i2c::{I2c, Operation};

/// Embedded HAL compatible driver for the INA219
pub struct INA219<I2C, Calib> {
    i2c: I2C,
    address: address::Address,
    #[cfg(feature = "paranoid")]
    config: Option<Configuration>,
    calib: Calib,
}

impl<I2C> INA219<I2C, UnCalibrated>
where
    I2C: I2c,
{
    /// Open an INA219 without calibration
    ///
    /// Performs a reset and if the `paranoid` feature is active checks all register values are in
    /// the expected ranges.
    ///
    /// # Errors
    /// If the device returns an unexpected response a `InitializationError` is returned.
    pub async fn new(
        i2c: I2C,
        address: address::Address,
    ) -> Result<Self, InitializationError<I2C, I2C::Error>> {
        Self::new_calibrated(i2c, address, UnCalibrated).await
    }
}

impl<I2C, Calib> INA219<I2C, Calib>
where
    I2C: I2c,
    Calib: Calibration,
{
    /// Open an INA219, perform a reset and check all register values are in the expected ranges than apply the provided calibration
    ///
    /// # Errors
    /// If the device returns an unexpected response a `InitializationError` is returned.
    pub async fn new_calibrated(
        i2c: I2C,
        address: address::Address,
        calibration: Calib,
    ) -> Result<Self, InitializationError<I2C, I2C::Error>> {
        let mut new = INA219::new_unchecked(i2c, address, calibration);

        // This is done in a function to make error handling easier...
        // since we want to return the device in case something goes wrong
        match new.init().await {
            Ok(()) => Ok(new),
            Err(e) => Err(InitializationError::new(e, new.destroy())),
        }
    }

    /// Perform the following steps on this device to bring it into a known state
    /// - Perform a Reset
    /// - Wait for the Reset to finish, by polling 10 times for if it is already done (are we there yet?)
    /// - If paranoid: Check if all registers are in the expected ranges
    /// - Apply the register value from self.calib
    async fn init(&mut self) -> Result<(), InitializationErrorReason<I2C::Error>> {
        self.reset().await?;

        // If we are paranoid we perform extra checks to verify we talk to a real INA219
        #[cfg(feature = "paranoid")]
        {
            use crate::calibration::RawCalibration;
            use crate::register::RegisterName;

            // read_configuration before should have populated the config which can now be used to
            // validate bus and shunt voltages
            assert!(self.config.is_some());

            // Check that all calculated registers read zero after reset
            if !matches!(self.read().await?, RawCalibration(0)) {
                return Err(InitializationErrorReason::RegisterNotZeroAfterReset(
                    RegisterName::Calibration,
                ));
            }

            if !matches!(self.read().await?, CurrentRegister(0)) {
                return Err(InitializationErrorReason::RegisterNotZeroAfterReset(
                    RegisterName::Current,
                ));
            }

            if !matches!(self.read().await?, PowerRegister(0)) {
                return Err(InitializationErrorReason::RegisterNotZeroAfterReset(
                    RegisterName::Power,
                ));
            }

            // Check that the shunt voltage is in range
            self.shunt_voltage().await?;

            // Check that the bus voltage is in range
            self.bus_voltage().await?;
        }

        // Calibrate the device
        let bits = self.calib.register_bits();
        if bits == 0 {
            // Do nothing
            // We can skip writing a calibration of 0 since that is the reset value
        } else {
            write(&mut self.i2c, self.address, &self.calib).await?;
        }

        Ok(())
    }

    /// Create a new `INA219` assuming the device is already initialized to the given values.
    ///
    /// This also does not write the given configuration or calibration.
    pub const fn new_unchecked(i2c: I2C, address: address::Address, calib: Calib) -> Self {
        INA219 {
            i2c,
            address,
            #[cfg(feature = "paranoid")]
            config: None,
            calib,
        }
    }

    /// Destroy the driver returning the underlying I2C device
    ///
    /// This does leave the device in it's current state.
    pub fn destroy(self) -> I2C {
        self.i2c
    }

    /// Perform a power-on-reset
    ///
    /// Make sure to set calibration after this finishes so self.calib matches what the device is
    /// calibrated to
    async fn reset(&mut self) -> Result<(), InitializationErrorReason<I2C::Error>> {
        const MAX_RESET_READ_RETRIES: u8 = 10;

        // Set the reset bit
        self.set_configuration(Configuration {
            reset: Reset::Reset,
            ..Default::default()
        })
        .await?;

        #[cfg(feature = "paranoid")]
        {
            self.config = None; // Reset is actually never read back, so it does not make sense to store it.
        }

        // Wait until the device reports that it is done
        let mut attempt = 0;
        loop {
            if self.read::<Configuration>().await? == Configuration::default() {
                #[cfg(feature = "paranoid")]
                {
                    self.config = Some(Configuration::default());
                }
                return Ok(());
            }

            if attempt > MAX_RESET_READ_RETRIES {
                return Err(InitializationErrorReason::ConfigurationNotDefaultAfterReset);
            }

            attempt += 1;
        }
    }

    /// Read the current [`Configuration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    ///
    /// *With feature `paranoid`*:
    ///
    /// If the read configuration does not match the last saved configuration an error is returned
    /// and the saved configuration is updated to the read configuration.
    pub async fn configuration(
        &mut self,
    ) -> Result<Configuration, ConfigurationReadError<I2C::Error>> {
        let read = self.read().await?;

        #[cfg(feature = "paranoid")]
        {
            let saved = *self.config.get_or_insert(read);
            if read != saved {
                self.config = Some(read);
                return Err(ConfigurationReadError::ConfigurationMismatch { read, saved });
            }
        }

        Ok(read)
    }

    /// Set a new [`Configuration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub async fn set_configuration(&mut self, conf: Configuration) -> Result<(), I2C::Error> {
        let result = self.write(conf).await;

        // TODO what to do in case this causes a reset? Just panic?

        #[cfg(feature = "paranoid")]
        {
            self.config = match result {
                Ok(()) => Some(conf),
                // We don't know anything about the current conf
                Err(_) => None,
            };
        }

        #[cfg_attr(not(feature = "paranoid"), allow(clippy::let_and_return))]
        result
    }

    /// Trigger a new measurement
    ///
    /// This reads the current configuration and writes it again. This causes a measurement to be made if the chip is in
    /// triggered mode. If it is in any other mode this does nothing.
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returned an error.
    pub async fn trigger(&mut self) -> Result<(), I2C::Error> {
        let config = {
            #[cfg(feature = "paranoid")]
            {
                self.config
            }
            #[cfg(not(feature = "paranoid"))]
            {
                None
            }
        };

        let old_config = match config {
            None => match self.configuration().await {
                Ok(c) => c,
                Err(ConfigurationReadError::I2cError(e)) => return Err(e),
                Err(ConfigurationReadError::ConfigurationMismatch { .. }) => unreachable!("This can only happen if we are paranoid and have stored a configuration. But in that case we never perform a read!"),
            },
            Some(c) => c,
        };

        self.set_configuration(old_config).await
    }

    /// Set a new [`Calibration`]
    ///
    /// # Errors
    /// Returns Err() when the underlying I2C device returns an error.
    pub async fn calibrate(&mut self, value: Calib) -> Result<(), I2C::Error> {
        self.calib = value;
        write(&mut self.i2c, self.address, &self.calib).await
    }

    /// Checks if a new measurement was performed since the last configuration change,
    /// [`Self::power_raw`] call or [`Self::next_measurement`] call returning Ok(None) if there is no new data
    ///
    /// TODO: Explain caveats around resetting the conversion ready flag
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when any of the
    /// measurements is outside of their expected ranges.
    #[allow(clippy::type_complexity)] // FIXME: Find a more elegant type
    pub async fn next_measurement(
        &mut self,
    ) -> Result<Option<Measurements<Calib::Current, Calib::Power>>, MeasurementError<I2C::Error>>
    {
        let (bus_voltage, power, shunt_voltage, current) = if Calib::READ_CURRENT {
            self.read4().await?
        } else {
            let (bus_voltage, power, shunt_voltage) = self.read3().await?;
            (bus_voltage, power, shunt_voltage, CurrentRegister(0))
        };

        let bus_voltage = self.bus_voltage_from_register(bus_voltage)?;
        if !bus_voltage.is_conversion_ready() {
            // No new data... nothing to do...
            return Ok(None);
        }

        let shunt_voltage = self.shunt_voltage_from_register(shunt_voltage)?;

        if bus_voltage.has_math_overflowed() {
            return Err(MeasurementError::MathOverflow(Measurements {
                bus_voltage,
                shunt_voltage,
                current: (),
                power: (),
            }));
        }

        Ok(Some(Measurements {
            bus_voltage,
            shunt_voltage,
            current: self.calib.current_from_register(current),
            power: self.calib.power_from_register(power),
        }))
    }

    /// Read the last measured shunt voltage
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when the shunt voltage
    /// is outside of the expected range given in the last written configuration.
    pub async fn shunt_voltage(
        &mut self,
    ) -> Result<ShuntVoltage, ShuntVoltageReadError<I2C::Error>> {
        let value: ShuntVoltageRegister = self.read().await?;

        self.shunt_voltage_from_register(value)
    }

    #[cfg_attr(not(feature = "paranoid"), allow(clippy::unused_self))]
    fn shunt_voltage_from_register(
        &mut self,
        value: ShuntVoltageRegister,
    ) -> Result<ShuntVoltage, ShuntVoltageReadError<I2C::Error>> {
        // If we are paranoid we look up what we last set for the full range
        #[cfg(feature = "paranoid")]
        let shunt_voltage_range = self
            .config
            .map_or(ShuntVoltageRange::Fsr320mv, |c| c.shunt_voltage_range);

        // If we are not paranoid we still check that it is in the maximum range
        #[cfg(not(feature = "paranoid"))]
        let shunt_voltage_range = ShuntVoltageRange::Fsr320mv;

        ShuntVoltage::from_bits_with_range(value, shunt_voltage_range).ok_or_else(|| {
            ShuntVoltageReadError::ShuntVoltageOutOfRange {
                should: shunt_voltage_range,
                is: ShuntVoltage::from_bits_unchecked(value),
            }
        })
    }

    /// Read the last measured bus voltage
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error or when the bus voltage
    /// is outside of the expected range given in the last written configuration.
    pub async fn bus_voltage(&mut self) -> Result<BusVoltage, BusVoltageReadError<I2C::Error>> {
        let value = self.read().await?;

        self.bus_voltage_from_register(value)
    }

    #[cfg_attr(not(feature = "paranoid"), allow(clippy::unused_self))]
    fn bus_voltage_from_register(
        &mut self,
        value: BusVoltageRegister,
    ) -> Result<BusVoltage, BusVoltageReadError<I2C::Error>> {
        // If we are paranoid we look up what we last set for the full range
        #[cfg(feature = "paranoid")]
        let bus_voltage_range = self
            .config
            .map_or(BusVoltageRange::Fsr32v, |c| c.bus_voltage_range);

        // If we are not paranoid we still check that it is in the maximum range
        #[cfg(not(feature = "paranoid"))]
        let bus_voltage_range = BusVoltageRange::Fsr32v;

        BusVoltage::from_bits_with_range(value, bus_voltage_range).ok_or_else(|| {
            BusVoltageReadError::BusVoltageOutOfRange {
                should: bus_voltage_range,
                is: BusVoltage::from_bits_unchecked(value),
            }
        })
    }

    /// Read the last measured power
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub async fn power_raw(&mut self) -> Result<PowerRegister, I2C::Error> {
        self.read().await
    }

    /// Read the last measured current
    ///
    /// # Errors
    /// Returns an error if the underlying I2C device returns an error.
    pub async fn current_raw(&mut self) -> Result<CurrentRegister, I2C::Error> {
        self.read().await
    }

    async fn read<Reg: register::ReadRegister>(&mut self) -> Result<Reg, I2C::Error> {
        let mut buf: [u8; 2] = [0x00; 2];
        self.i2c
            .write_read(self.address.as_byte(), &[Reg::ADDRESS], &mut buf)
            .await?;
        Ok(Reg::from_bits(u16::from_be_bytes(buf)))
    }

    read_many!(read3, (R0, b0), (R1, b1), (R2, b2));
    read_many!(read4, (R0, b0), (R1, b1), (R2, b2), (R3, b3));

    /// Write the value contained in the register to the address dictated by its type
    async fn write(&mut self, reg: impl WriteRegister + Copy) -> Result<(), I2C::Error> {
        write(&mut self.i2c, self.address, &reg).await
    }
}

// Since I do not want restrict calibration to be Clone we need a way to call write without having
// to give out both &mut self and &self
async fn write<I2C: I2c, Reg: WriteRegister>(
    dev: &mut I2C,
    addr: Address,
    value: &Reg,
) -> Result<(), I2C::Error> {
    let [val0, val1] = value.as_bits().to_be_bytes();
    dev.write(addr.as_byte(), &[Reg::ADDRESS, val0, val1]).await
}

macro_rules! read_many {
    ($name:ident, $(($reg:ident, $buf:ident)),+) => {
        async fn $name<$($reg),+>(&mut self) -> Result<($($reg,)+), I2C::Error>
        where
            $($reg: register::ReadRegister),+
        {
            $(let mut $buf: [u8; 2] = [0x00; 2];)+
            if cfg!(feature = "no_transaction") {
                let addr = self.address.as_byte();
                $(self.i2c.write_read(addr, &[$reg::ADDRESS], &mut $buf).await?;)+
            } else {
                let mut transactions = [
                    $(Operation::Write(&[$reg::ADDRESS]), Operation::Read(&mut $buf),)+
                ];
                self.i2c
                    .transaction(self.address.as_byte(), &mut transactions[..])
                    .await?;
            }

            Ok(($($reg::from_bits(u16::from_be_bytes($buf)),)+))
        }
    };
}
use read_many;
