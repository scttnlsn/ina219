#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(unused_assignments)]

//ElectricCurrent is a measurement of a flow of electric charge stored as an int64 nano Ampere.
pub type ElectricCurrent = i64;
pub const NanoAmpere: ElectricCurrent = 1;
pub const MicroAmpere: ElectricCurrent = 1000 * NanoAmpere;
pub const MilliAmpere: ElectricCurrent = 1000 * MicroAmpere;
pub const Ampere     : ElectricCurrent = 1000 * MilliAmpere;
pub const KiloAmpere : ElectricCurrent = 1000 * Ampere;
pub const MegaAmpere : ElectricCurrent = 1000 * KiloAmpere;
pub const GigaAmpere : ElectricCurrent = 1000 * MegaAmpere;

//ElectricPotential is a measurement of electric potential stored as an int64 nano Volt.
pub type  ElectricPotential = i64;
// Volt is W/A, kg⋅m²/s³/A.
pub const NanoVolt :  ElectricPotential = 1;
pub const MicroVolt:  ElectricPotential = 1000 * NanoVolt;
pub const MilliVolt:  ElectricPotential = 1000 * MicroVolt;
pub const Volt     :  ElectricPotential = 1000 * MilliVolt;
pub const KiloVolt :  ElectricPotential = 1000 * Volt;
pub const MegaVolt :  ElectricPotential = 1000 * KiloVolt;
pub const GigaVolt :  ElectricPotential = 1000 * MegaVolt;

//ElectricResistance is a measurement of the difficulty to pass an electric current through a conductor stored as an int64 nano Ohm.
pub type ElectricResistance = i64;
// Ohm is V/A, kg⋅m²/s³/A².
pub const NanoOhm : ElectricResistance = 1;
pub const MicroOhm: ElectricResistance = 1000 * NanoOhm;
pub const MilliOhm: ElectricResistance = 1000 * MicroOhm;
pub const Ohm     : ElectricResistance = 1000 * MilliOhm;
pub const KiloOhm : ElectricResistance = 1000 * Ohm;
pub const MegaOhm : ElectricResistance = 1000 * KiloOhm;
pub const GigaOhm : ElectricResistance = 1000 * MegaOhm;


//Power is a measurement of  Power stored as a nano watts.
pub type  Power = i64;
	// Watt is unit of Power J/s, kg⋅m²⋅s⁻³
pub const NanoWatt  : Power = 1;
pub const MicroWatt : Power = 1000 * NanoWatt;
pub const MilliWatt : Power = 1000 * MicroWatt;
pub const Watt      : Power = 1000 * MilliWatt;
pub const KiloWatt  : Power = 1000 * Watt;
pub const MegaWatt  : Power = 1000 * KiloWatt;
pub const GigaWatt  : Power = 1000 * MegaWatt;


pub fn nanoAsString(mut v: i64) -> String {
	let mut sign: String = String::from("");
	if v < 0 {
		if v == -9223372036854775808 {
			v = v+1;
		}
		sign = String::from("-");
		v = -v;
	}
	let mut frac: i32 = Default::default();
	let mut base: i32 = Default::default();
	let mut precision: i64 = Default::default();
	let mut unit: String = String::from("");
	let value_option = Option::Some(v);
	match value_option {
		Some(v) if v >= 999999500000000001 => {
			precision = v % 1000000000000000;
			base = (v / 1000000000000000) as i32;
			if precision > 500000000000000 {
				base = base + 1;
			}
			frac = base % 1000;
			base = base / 1000;
			unit = String::from("G");
		},
		Some(v) if  v >= 999999500000001 => {
			precision = v % 1000000000000;
			base = (v / 1000000000000) as i32;
			if precision > 500000000000 {
				base = base +1;
			}
			frac = base % 1000;
			base = base / 1000;
			unit = String::from("M");
		},
		Some(v) if v >= 999999500001 => {
			precision = v % 1000000000;
			base = (v / 1000000000) as i32;
			if precision > 500000000 {
				base = base + 1;
			}
			frac = base % 1000;
			base = base / 1000;
			unit =  "k".to_string();
		},
		Some(v) if  v >= 999999501 => {
			precision = v % 1000000;
			base = (v / 1000000) as i32;
			if precision > 500000 {
				base = base + 1;
			}
			frac = base % 1000;
			base = base / 1000;
			unit = "".to_string();
		},
		Some(v) if  v >= 1000000 => {
			precision = v % 1000;
			base = (v / 1000) as i32;
			if precision > 500 {
				base = base + 1;
			}
			frac = base % 1000;
			base = base / 1000;
			unit = "m".to_string();
		},
		Some(v) if v >= 1000 => {
			frac = (v as i32) % 1000;
			base = (v as i32) / 1000;
			unit = "µ".to_string();
		},
		Some(v) if 0 <  v && v < 1000 => {
			base = v as i32;
			unit = "n".to_string();
		},
		Some(v) if v == 0 => {
			return "0".to_string();
		},
		None => {
		},
		_ => panic!(),
	}

	if frac == 0 {
		 return sign + &base.to_string() + &unit;
	}
	return sign + &base.to_string() + &".".to_string() + &prefixZeros(3, frac) + &unit;
}


pub fn prefixZeros(digits: i32, v: i32) -> String {
	let mut s = v.to_string();
	let mut str_len = s.len() as i32;
	while str_len < digits {
		s = "0".to_string() + &s;
		str_len += 1;

	}
	return s;
}