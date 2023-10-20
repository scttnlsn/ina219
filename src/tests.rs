use crate::address::Address;
use crate::calibration::{IntCalibration, MicroAmpere, UnCalibrated};
use crate::configuration::{BusVoltageRange, ShuntVoltageRange};
use crate::errors::{BusVoltageReadError, MeasurementError, ShuntVoltageReadError};
use crate::measurements::Measurements;
use crate::{Register, INA219};
use embedded_hal_mock::eh1::i2c::{Mock as I2cMock, Transaction};

const DEV_ADDR: u8 = 0x40;

#[allow(clippy::cast_possible_truncation)]
fn read_reg(reg: Register, value: u16) -> Transaction {
    Transaction::write_read(
        DEV_ADDR,
        vec![reg as u8],
        vec![(value >> 8) as u8, value as u8],
    )
}

#[allow(clippy::cast_possible_truncation)]
fn write_reg(reg: Register, value: u16) -> Transaction {
    Transaction::write(DEV_ADDR, vec![reg as u8, (value >> 8) as u8, value as u8])
}

fn init_transactions() -> Vec<Transaction> {
    use Register::{BusVoltage, Calibration, Configuration, Current, Power, ShuntVoltage};

    vec![
        write_reg(Configuration, 0b1011_1001_1001_1111),
        read_reg(Configuration, 0b0011_1001_1001_1111),
        //
        read_reg(Calibration, 0),
        read_reg(Current, 0),
        read_reg(Power, 0),
        //
        read_reg(ShuntVoltage, 0),
        read_reg(BusVoltage, 0),
    ]
}

fn mock_uncal(transactions: &[Transaction]) -> INA219<I2cMock, UnCalibrated> {
    let mut all_transactions = init_transactions();
    all_transactions.extend_from_slice(transactions);
    let mock = I2cMock::new(&all_transactions);

    INA219::new(mock, Address::default(), UnCalibrated).unwrap()
}

fn mock_cal(transactions: &[Transaction]) -> INA219<I2cMock, IntCalibration> {
    let mut all_transactions = init_transactions();
    all_transactions.push(write_reg(Register::Calibration, 409 & !1));
    all_transactions.extend_from_slice(transactions);
    let mock = I2cMock::new(&all_transactions);

    INA219::new(
        mock,
        Address::default(),
        IntCalibration::new(MicroAmpere(100), 1_000_000).unwrap(),
    )
    .unwrap()
}

#[test]
fn initialization() {
    let ina = mock_uncal(&[]);
    ina.destroy().done();
}

#[test]
fn initialization_cal() {
    let ina = mock_cal(&[]);
    ina.destroy().done();
}

#[test]
fn read_measurements() {
    let mut ina = mock_uncal(&[
        read_reg(Register::BusVoltage, 0x0FA0 << 3 | 0b10),
        read_reg(Register::Power, 0),
        read_reg(Register::ShuntVoltage, 0b0001_1111_0100_0000),
    ]);

    let m = ina.next_measurement().unwrap().unwrap();
    assert_eq!(m.shunt_voltage.shunt_voltage_mv(), 80);
    assert_eq!(m.bus_voltage.voltage_mv(), 16_000);

    ina.destroy().done();
}

#[test]
fn read_measurements_with_cal() {
    let mut ina = mock_cal(&[
        read_reg(Register::BusVoltage, 0x0FA0 << 3 | 0b10),
        read_reg(Register::Power, 636),
        read_reg(Register::ShuntVoltage, 0b0001_1111_0100_0000),
        read_reg(Register::Current, 796),
    ]);

    let m = ina.next_measurement().unwrap().unwrap();
    assert_eq!(m.shunt_voltage.shunt_voltage_mv(), 80);
    assert_eq!(m.bus_voltage.voltage_mv(), 16_000);

    // These should be 80mV and 1280mW, but because of the slight error in the calibration they come
    // out slightly different.
    assert_eq!(m.current.0, 79_600);
    assert_eq!(m.power.0, 1_272_000);

    ina.destroy().done();
}

#[test]
fn math_overflow() {
    let mut ina = mock_cal(&[
        read_reg(Register::BusVoltage, 0x0FA0 << 3 | 0b11),
        read_reg(Register::Power, 636),
        read_reg(Register::ShuntVoltage, 0b0001_1111_0100_0000),
    ]);

    let err = ina.next_measurement().unwrap_err();
    match err {
        MeasurementError::MathOverflow(Measurements {
            bus_voltage,
            shunt_voltage,
            ..
        }) => {
            assert_eq!(bus_voltage.voltage_mv(), 16_000);
            assert_eq!(shunt_voltage.shunt_voltage_mv(), 80);
        }
        _ => panic!("Unexpected error: {err:?}"),
    }

    ina.destroy().done();
}

#[test]
fn bus_out_of_range_values() {
    let mut ina = mock_cal(&[read_reg(Register::BusVoltage, 8_001 << 3 | 0b10)]);

    match ina.bus_voltage().unwrap_err() {
        BusVoltageReadError::BusVoltageOutOfRange { should, is } => {
            assert_eq!(is.voltage_mv(), 32_004);
            assert_eq!(should, BusVoltageRange::Fsr32v);
        }
        e @ BusVoltageReadError::I2cError(_) => panic!("Unexpected error:{e:?}"),
    }

    ina.destroy().done();
}

#[test]
fn shunt_out_of_range_values() {
    let mut ina = mock_cal(&[read_reg(Register::ShuntVoltage, 32_001)]);

    match ina.shunt_voltage().unwrap_err() {
        ShuntVoltageReadError::ShuntVoltageOutOfRange { should, is } => {
            assert_eq!(is.shunt_voltage_mv(), 320);
            assert_eq!(should, ShuntVoltageRange::Fsr320mv);
        }
        e @ ShuntVoltageReadError::I2cError(_) => panic!("Unexpected error: {e:?}"),
    }

    ina.destroy().done();
}
