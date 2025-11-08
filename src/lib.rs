#![no_std]
pub mod uart;
pub mod led;

// clock speeds of base clocks
pub const SYSOSC_FREQUENCY: u32 = 32_000_000;
pub const LFCLK_FREQUENCY: u32 = 32_000;
pub const MFCLK_FREQUENCY: u32 = 4_000_000;
pub const RTCCLK_FREQUENCY: u32 = 32_000;

pub use mspm0l222x_pac::Peripherals;

