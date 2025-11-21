#![no_std]
pub mod iomux;
pub mod led;
pub mod uart;

// clock speeds of base clocks
pub const SYSOSC_FREQUENCY: u32 = 32_000_000;
pub const LFCLK_FREQUENCY: u32 = 32_000;
pub const MFCLK_FREQUENCY: u32 = 4_000_000;
pub const RTCCLK_FREQUENCY: u32 = 32_000;

pub const PWREN_WRITE_KEY: u32 = 0x26000000;
pub const RSTCTL_WRITE_KEY: u32 = 0xB1000000;

pub use mspm0l222x_pac::{self, Peripherals};
