use core::fmt::{self, Write};
use cortex_m::asm::nop;
use once_cell::sync::OnceCell;
use thiserror::Error;

use crate::cursor::Cursor;
use crate::iomux::Iomux;
use crate::{HalError, PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

const UART_FREQUENCY: u32 = 115200;

static UART0: OnceCell<Uart0> = OnceCell::new();
static UART1: OnceCell<Uart1> = OnceCell::new();

/// Initializes uart, creating and returning an instance of Uart; panic if the initialization fails.
pub fn uart0() -> &'static Uart0 {
    UART0.get().expect("uart0 not yet initialized")
}
/// Global instance for UART1
pub fn uart1() -> &'static Uart1 {
    UART1.get().expect("uart1 not yet initialized")
}

// Taken from ti/driverlib/dl_uart.c
const fn divisor(freq: u32) -> u32 {
    ((crate::SYSOSC_FREQUENCY * 8) / freq).div_ceil(2)
}

#[derive(Error, Debug)]
pub enum UartError {
    #[error("Read error; failed after reading {0} bytes ({1:?}, {2})")]
    ReadError(usize, ReadErrorKind, &'static str),
    #[error("Write error")]
    WriteError,
}

#[derive(Debug)]
pub enum ReadErrorKind {
    BrkErr,
    FrmErr,
    NErr,
    OvrErr,
    ParErr,
}

macro_rules! uart_impl {
    ($n:literal, $tx_iomux:literal, $rx_iomux:literal, $pf:literal) => {
        pub struct ${concat(Uart, $n)} {
            regs: mspm0l222x_pac::${concat(Uart, $n)},
        }

        // TODO: is this fine?
        unsafe impl Sync for ${concat(Uart, $n)} {}

        impl ${concat(Uart, $n)} {
            pub fn new(iomux: &Iomux, uart: mspm0l222x_pac::${concat(Uart, $n)}, freq: u32) -> Self {
                // Disable UART before configuration
                uart.${concat(uart, $n, _gprcm)}(0).${concat(uart, $n, _rstctl)}().write(|w| {
                    unsafe { w.bits(RSTCTL_WRITE_KEY) }
                        .resetassert()
                        .assert()
                        .resetstkyclr()
                        .clr()
                });

                uart.${concat(uart, $n, _gprcm)}(0)
                    .${concat(uart, $n, _pwren)}()
                    .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());

                // delay while UART initializes
                for _ in core::hint::black_box(0..32) {
                    nop();
                }

                // set up IOMUX to output
                iomux.connect_pin($tx_iomux, $pf);
                iomux.connect_pin($rx_iomux, $pf);

                // disable UART
                uart.${concat(uart, $n, _ctl0)}().write(|w| w.enable().clear_bit());

                // Select clock source (BUSCLK) and divisor
                uart.${concat(uart, $n, _clksel)}().write(|w| w.busclk_sel().enable());
                uart.${concat(uart, $n, _clkdiv)}().write(|w| w.ratio().div_by_1());

                // Set baud rate divisors
                let div = divisor(freq);
                uart.${concat(uart, $n, _ibrd)}()
                    .write(|w| unsafe { w.divint().bits((div >> 6) as u16) });
                uart.${concat(uart, $n, _fbrd)}()
                    .write(|w| unsafe { w.divfrac().bits((div & 0b111111) as u8) });

                // set all UART settings
                uart.${concat(uart, $n, _ctl0)}().write(|w| {
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
                uart.${concat(uart, $n, _lcrh)}()
                    .write(|w| w.pen().disable().wlen().databit8().stp2().disable());

                // enable UART
                uart.${concat(uart, $n, _ctl0)}().modify(|_, w| w.enable().enable());

                Self { regs: uart }
            }

            pub fn write_bytes(&self, bytes: &[u8]) {
                let mut bytes = bytes;
                while let Some((head, tail)) = bytes.split_first() {
                    if self.regs.${concat(uart, $n, _stat)}().read().txff().bit_is_clear() {
                        self.regs
                            .${concat(uart, $n, _txdata)}()
                            .write(|w| unsafe { w.data().bits(*head) });
                        bytes = tail;
                    }
                }
                // wait for data to flush
                while self.regs.${concat(uart, $n, _stat)}().read().txfe().is_cleared() {
                    nop();
                }
            }

            pub fn read_bytes(&self, bytes: &mut [u8]) -> Result<(), HalError> {
                for (i, b) in bytes.iter_mut().enumerate() {
                    while self.regs.${concat(uart, $n, _stat)}().read().rxfe().bit_is_set() {}
                    let result = self.regs.${concat(uart, $n, _rxdata)}().read();

                    let err = match () {
                        _ if result.brkerr().bit_is_set() => Some(ReadErrorKind::BrkErr),
                        _ if result.frmerr().bit_is_set() => Some(ReadErrorKind::FrmErr),
                        _ if result.nerr().bit_is_set() => Some(ReadErrorKind::NErr),
                        _ if result.ovrerr().bit_is_set() => Some(ReadErrorKind::OvrErr),
                        _ if result.parerr().bit_is_set() => Some(ReadErrorKind::ParErr),
                        _ => None
                    };
                    if let Some(kind) = err {
                        return Err(UartError::ReadError(i, kind, core::any::type_name::<Self>()).into());
                    }

                    *b = result.data().bits();
                }

                Ok(())
            }

            pub fn read_byte(&self) -> Result<u8, HalError> {
                let mut value = 0u8;
                self.read_bytes(bytemuck::bytes_of_mut(&mut value))?;
                Ok(value)
            }

            /// Returns whether either tx or rx is busy
            pub fn busy(&self) -> bool {
                self.regs.${concat(uart, $n, _stat)}().read().busy().bit_is_set()
            }
        }
    };
}

// Uart{n}, tx iomux, rx iomux, iomux pin function (pf)
uart_impl!(0, 25, 26, 2);
impl Uart0 {
    pub fn init(iomux: &Iomux, uart: mspm0l222x_pac::Uart0) {
        let _ = UART0.get_or_init(|| Uart0::new(iomux, uart, UART_FREQUENCY));
    }
}

// some UARTs can be connected to alternate pins (e.g. uart1 can also be connected to pincm10/pincm11)
// these only reflect one configuration
// TODO(wondering): check that this configuration works
uart_impl!(1, 8, 9, 10);
impl Uart1 {
    pub fn init(iomux: &Iomux, uart: mspm0l222x_pac::Uart1) {
        let _ = UART1.get_or_init(|| Uart1::new(iomux, uart, UART_FREQUENCY));
    }
}
uart_impl!(2, 39, 40, 10);
uart_impl!(3, 40, 39, 4);
uart_impl!(4, 31, 32, 6);

pub fn write_debug_format(args: fmt::Arguments) {
    let mut message_buf = [0; 256];

    let mut cursor = Cursor::new(&mut message_buf);
    cursor.write_fmt(args).unwrap();
    let message_len = cursor.offset;

    uart0().write_bytes(&message_buf[..message_len]);
}

/// Prints to the uart port
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::uart::write_debug_format(format_args!($($arg)*)));
}

/// Prints to the uart port
#[macro_export]
macro_rules! println {
    () => ($crate::uart::print!("\n"));
    ($($arg:tt)*) => ($crate::uart::print!("{}\n", format_args!($($arg)*)));
}

pub use {print, println};
