#![no_std]

use crc::{Crc, CRC_16_IBM_SDLC};

static X25: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

pub static CHUNK_SIZE: usize = 256;

pub static SPLASH: &str = r#"
 _______       _                _          _
|__   __|     | |              | |        | |
   | |_ __ ___| |__  _   _  ___| |__   ___| |_
   | | '__/ _ \ '_ \| | | |/ __| '_ \ / _ \ __|
   | | | |  __/ |_) | |_| | (__| | | |  __/ |_
   |_|_|  \___|_.__/ \__,_|\___|_| |_|\___|\__|

        Trebuchet: the StoneOS chain-loader
"#;

pub fn checksum(msg: &[u8]) -> u16 {
    X25.checksum(msg)
}
