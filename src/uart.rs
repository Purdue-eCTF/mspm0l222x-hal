use core::fmt::{self, Write};
use cortex_m::asm::nop;
use mspm0l222x_pac::{Iomux, Uart0};
use once_cell::sync::OnceCell;
use thiserror::Error;

use crate::cursor::Cursor;
use crate::{HalError, PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

const UART_FREQUENCY: u32 = 9600; /* (baud) rate data is transferred */
const UART_FUNCTION: u8 = 2;
const TX_IOMUX: usize = 25 - 1;
const RX_IOMUX: usize = 26 - 1;

static UART: OnceCell<Uart> = OnceCell::new();

/// Initializes uart, creating and returning an instance of Uart.
/// If the initialization fails, panic with an error message.
pub fn uart() -> &'static Uart {
    UART.get().expect("uart not yet initialized")
}

/// Taken from ti/driverlib/dl_uart.c.
const fn divisor(freq: u32) -> u32 {
    ((crate::SYSOSC_FREQUENCY * 8) / freq).div_ceil(2)
}

/// Register for the UART, which is used for serial communication.
pub struct Uart {
    regs: Uart0,
}

/// The error that is returned in the failure of a read or write operation.
#[derive(Error, Debug)]
pub enum UartError {
    /// An error has occured while reading; returns the number of bytes that have been successfully
    /// read.
    #[error("Read error; failed after reading {0} bytes")]
    ReadError(usize),
    /// An error has occured while writing.
    #[error("Write error")]
    WriteError,
}

// TODO: is this fine?
unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

impl Uart {
    /// Initate the hardware UART if it isn't already initiated before retrieving the uart.
    pub fn init(iomux: &Iomux, uart: Uart0) {
        let _ = UART.get_or_init(|| Uart::new(iomux, uart, UART_FREQUENCY));
    }

    /// Configure the new UART. (see Reference Manual section 21.2.6)
    fn new(iomux: &Iomux, uart: Uart0, freq: u32) -> Self {
        // Disable UART before configuration
        uart.uart0_gprcm(0).uart0_rstctl().write(|w| {
            unsafe { w.bits(RSTCTL_WRITE_KEY) }
                .resetassert()
                .assert()
                .resetstkyclr()
                .clr()
        });

        // Enable power for uart with PWREN register
        uart.uart0_gprcm(0)
            .uart0_pwren()
            .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());

        // delay while UART initializes
        for _ in core::hint::black_box(0..32) {
            nop();
        }

        // set up IOMUX to output (TX pin)
        iomux
            .iomux_pincm(TX_IOMUX)
            .write(|w| unsafe { w.pf().bits(UART_FUNCTION).pc().connected() });
        // set up IOMUX to input (RX pin)
        iomux
            .iomux_pincm(RX_IOMUX)
            .write(|w| unsafe { w.pf().bits(UART_FUNCTION).pc().connected() });

        // disable UART
        uart.uart0_ctl0().write(|w| w.enable().clear_bit());

        // Select clock source (BUSCLK) with (clksel) and divisor with (clkdiv)
        uart.uart0_clksel().write(|w| w.busclk_sel().enable());
        uart.uart0_clkdiv().write(|w| w.ratio().div_by_1());

        // Set baud rate divisors
        let div = divisor(freq);
        uart.uart0_ibrd() // set to integer part of the BRD, INT(BRD),
            .write(|w| unsafe { w.divint().bits((div >> 6) as u16) });
        uart.uart0_fbrd() // set to BRD % 64, fractional part
            .write(|w| unsafe { w.divfrac().bits((div & 0b111111) as u8) });

        // set all UART settings
        uart.uart0_ctl0().write(|w| {
            w.hse()
                .ovs16()
                .fen()
                .enable()
                .txe()
                .enable()
                .rxe()
                .enable()
                .mode()
                .uart()
                .ctsen()
                .disable()
                .rtsen()
                .disable()
        });

        // Configure line control
        uart.uart0_lcrh()
            .write(|w| w.pen().disable().wlen().databit8().stp2().disable());

        // enable UART
        uart.uart0_ctl0().modify(|_, w| w.enable().enable());

        Self { regs: uart }
    }

    /// Write bytes to TX.
    pub fn write_bytes(&self, bytes: &[u8]) {
        let mut bytes = bytes;
        while let Some((head, tail)) = bytes.split_first() {
            if self.regs.uart0_stat().read().txff().bit_is_clear() {
                self.regs
                    .uart0_txdata()
                    .write(|w| unsafe { w.data().bits(*head) });
                bytes = tail;
            }
        }
        // wait for data to flush
        while self.regs.uart0_stat().read().txfe().is_cleared() {
            nop();
        }
    }

    /// Read bytes from RX and returns a Result.
    pub fn read_bytes(&self, bytes: &mut [u8]) -> Result<(), HalError> {
        for (i, b) in bytes.iter_mut().enumerate() {
            while self.regs.uart0_stat().read().rxfe().bit_is_clear() {}
            let result = self.regs.uart0_rxdata().read();
            if result.brkerr().bit_is_set()
                || result.frmerr().bit_is_set()
                || result.nerr().bit_is_set()
                || result.ovrerr().bit_is_set()
                || result.parerr().bit_is_set()
            {
                return Err(UartError::ReadError(i).into());
            }
            *b = result.data().bits();
        }

        Ok(())
    }

    /// Returns whether either TX or RX is busy
    pub fn busy(&self) -> bool {
        self.regs.uart0_stat().read().busy().bit_is_set()
    }
}

/// TODO AFTER CURSOR (nvm I did cursor and I still don't really get it)
pub fn write_debug_format(args: fmt::Arguments) {
    let mut message_buf = [0; 256];

    let mut cursor = Cursor::new(&mut message_buf);
    cursor.write_fmt(args).unwrap();
    let message_len = cursor.offset;

    uart().write_bytes(&message_buf[..message_len]);
}

/// Prints to the uart port.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::uart::write_debug_format(format_args!($($arg)*)));
}

/// Prints to the uart port with newline.
#[macro_export]
macro_rules! println {
    () => ($crate::uart::print!("\n"));
    ($($arg:tt)*) => ($crate::uart::print!("{}\n", format_args!($($arg)*)));
}

pub use {print, println};
