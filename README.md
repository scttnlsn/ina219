# ina219

[![crates.io](https://img.shields.io/crates/v/ina219.svg)](https://crates.io/crates/ina219)

Blocking and async driver for the [INA219](http://www.ti.com/product/INA219) current/power monitor by Texas Instruments.



## Features
This crate has the following feature flags (default features in bold):

* ***sync***: Provide a blocking driver implementation
* ***async***: Provide an async driver implementation
* ***paranoid***: Perform extra checks
* *no_transaction*: Disable use of transactions and perform individual system calls
* *std*: Use the standard library and impl std::error::Error on all error types

For more detailed descriptions see [Cargo.toml](Cargo.toml).

## Calibration
This driver includes ways to use the calibration feature of the INA219. However, the errors introduced by the 
calculations can be unintuitive. So it can make sense to just compute the current and power in software.

## Examples
The [examples](examples/) folder contains code that demonstrates how this driver can be used. They were tested on a
Raspberry Pi with an INA219 that was configured for address 0x42.