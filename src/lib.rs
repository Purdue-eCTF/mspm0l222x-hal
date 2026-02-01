#![no_std]
use thiserror::Error;

pub mod cursor;
pub mod flash;
pub mod iomux;
pub mod led;
pub mod mpu;
pub mod trng;
pub mod uart;
pub mod aes;

// clock speeds of base clocks
pub const SYSOSC_FREQUENCY: u32 = 32_000_000;
pub const LFCLK_FREQUENCY: u32 = 32_000;
pub const MFCLK_FREQUENCY: u32 = 4_000_000;
pub const RTCCLK_FREQUENCY: u32 = 32_000;

pub const PWREN_WRITE_KEY: u32 = 0x26000000;
pub const RSTCTL_WRITE_KEY: u32 = 0xB1000000;

pub const FLASH_SIZE: u32 = 1 << 15;
pub const SRAM_SIZE: u32 = 1 << 18;

pub use mspm0l222x_pac::{self, Peripherals};
pub use trng::Trng;

#[derive(Debug, Error)]
pub enum HalError {
    #[error("Unknown error")]
    Unknown,
    #[error("Flash error: {0}")]
    FlashError(#[from] flash::FlashError),
    #[error("Uart error: {0}")]
    UartError(#[from] uart::UartError),
    #[error("MPU error error: {0}")]
    MpuError(#[from] mpu::MpuError),
}