#![no_std]
use thiserror::Error;

/// TODO Cursor for reading/writing to buffers.
pub mod cursor;
/// The flash is non-volatile memory (NVM) with 1k size sectors and total 256KiB memory.
pub mod flash;
/// The IOMUX controls and connects digital pins to peripheral pins.
pub mod iomux;
/// Two LEDs that can be set to red, green, or blue.
pub mod led;
/// A memory protection unit (MPU) that ensures safety while setting the SRAM RW and RX regions.
pub mod mpu;
/// A true random number generator (TRNG) block.
pub mod trng;
/// Facilitates serial communication by TX and RX pins.
pub mod uart;

// clock speeds of base clocks
/// The system oscillator frequency is 32MHz. (Reference Manual 2.3.2.1)
pub const SYSOSC_FREQUENCY: u32 = 32_000_000;
/// The low frequency clock frequency is is 32kHz. (Reference Manual 2.3.2.6)
pub const LFCLK_FREQUENCY: u32 = 32_000;
/// The middle frequency clock frequency is 4MHz. (Reference Manual 2.3.2.4)
pub const MFCLK_FREQUENCY: u32 = 4_000_000;
/// The real-time clock freqeuncy matches the low frequency clock, being 32kHz. (Reference Manual 2.3.2.10)
pub const RTCCLK_FREQUENCY: u32 = 32_000;

/// Key for register that controls power state.
pub const PWREN_WRITE_KEY: u32 = 0x26000000;
/// Key for register that controls reset assertion and de-assertion.
pub const RSTCTL_WRITE_KEY: u32 = 0xB1000000;

/// TODO The flash size is 256KiB.
pub const FLASH_SIZE: u32 = 1 << 15;
/// The SRAM size is 32KiB.
pub const SRAM_SIZE: u32 = 1 << 18;

pub use mspm0l222x_pac::{self, Peripherals};
pub use trng::Trng;

/// The error that is returned when something has gone wrong with the HAL, whether in flash, uart,
/// mpu, or with an unknown cause.
#[derive(Debug, Error)]
pub enum HalError {
    /// The error is unknown...
    #[error("Unknown error")]
    Unknown,
    /// There was an error during an operation with the flash.
    #[error("Flash error: {0}")]
    FlashError(#[from] flash::FlashError),
    /// There was an error during an operation with the uart.
    #[error("Uart error: {0}")]
    UartError(#[from] uart::UartError),
    /// TODO There was an error error during an operation with the mpu.
    #[error("MPU error error: {0}")]
    MpuError(#[from] mpu::MpuError),
}
