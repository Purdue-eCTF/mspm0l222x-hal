use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};
use cortex_m::asm::nop;
use mspm0l222x_pac::{Iomux, Uart0};

use crate::{PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

static UART_SETUP: AtomicBool = AtomicBool::new(false);

const UART_FREQUENCY: u32 = 115200;
const TX_IOMUX: usize = 25 - 1;
const RX_IOMUX: usize = 26 - 1;

// Taken from ti/driverlib/dl_uart.c
const fn divisor(freq: u32) -> u32 {
    ((crate::SYSOSC_FREQUENCY * 8) / freq + 1) / 2
}

use mspm0l222x_pac::uart0::uart0_ctl0::{Ctsen, Fen, Hse, Mode, Rtsen, Rxe, Txe};
use mspm0l222x_pac::uart0::uart0_lcrh::{Pen, Stp2, Wlen};

pub struct UartOptions {
    hse: Hse,
    fen: Fen,
    txe: Txe,
    rxe: Rxe,
    mode: Mode,
    ctsen: Ctsen,
    rtsen: Rtsen,
    uart_freq: u32,
    pen: Pen,
    wlen: Wlen,
    stp2: Stp2,
}

impl Default for UartOptions {
    fn default() -> Self {
        Self {
            hse: mspm0l222x_pac::uart0::uart0_ctl0::Hse::Ovs16,
            fen: Fen::Enable,
            txe: Txe::Enable,
            rxe: Rxe::Enable,
            mode: Mode::Uart,
            ctsen: Ctsen::Disable,
            rtsen: Rtsen::Disable,
            uart_freq: UART_FREQUENCY,
            pen: Pen::Disable,
            wlen: Wlen::Databit8,
            stp2: Stp2::Disable,
        }
    }
}

pub struct Uart<'a> {
    regs: &'a Uart0,
}
impl<'a> Uart<'a> {
    pub fn new_with_config(opts: UartOptions, iomux: &Iomux, uart: &'a Uart0) -> Self {
        if !UART_SETUP.load(Ordering::SeqCst) {
            // set up IOMUX to output
            iomux
                .iomux_pincm(TX_IOMUX)
                .write(|w| unsafe { w.pf().bits(2) });

            iomux
                .iomux_pincm(RX_IOMUX)
                .write(|w| unsafe { w.pf().bits(2) });

            // Disable UART before configuration
            uart.uart0_gprcm(0).uart0_rstctl().write(|w| {
                unsafe { w.bits(RSTCTL_WRITE_KEY) }
                    .resetassert()
                    .assert()
                    .resetstkyclr()
                    .clr()
            });

            uart.uart0_gprcm(0)
                .uart0_pwren()
                .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());
            // TODO: wait several cycles? does this need to be changed?
            for _ in core::hint::black_box(0..32) {}

            // Select clock source (BUSCLK) and divisor
            uart.uart0_clksel().write(|w| w.busclk_sel().enable());
            uart.uart0_clkdiv().write(|w| w.ratio().div_by_1());
            // disable UART
            uart.uart0_ctl0().write(|w| w.enable().clear_bit());

            // Set baud rate divisors
            let div = divisor(opts.uart_freq);
            uart.uart0_ibrd()
                .write(|w| unsafe { w.divint().bits((div >> 6) as u16) });
            uart.uart0_fbrd()
                .write(|w| unsafe { w.divfrac().bits((div & 0b111111) as u8) });

            // set all UART settings
            uart.uart0_ctl0().write(|w| {
                w.hse()
                    .variant(opts.hse)
                    .fen()
                    .variant(opts.fen)
                    .txe()
                    .variant(opts.txe)
                    .rxe()
                    .variant(opts.rxe)
                    .mode()
                    .variant(opts.mode)
                    .ctsen()
                    .variant(opts.ctsen)
                    .rtsen()
                    .variant(opts.rtsen)
            });

            // Configure line control
            uart.uart0_lcrh().write(|w| {
                w.pen()
                    .variant(opts.pen)
                    .wlen()
                    .variant(opts.wlen)
                    .stp2()
                    .variant(opts.stp2)
            });

            // enable UART
            // if you don't set fen here, it resets to 0. why? who knows
            uart.uart0_ctl0()
                .write(|w| w.enable().enable().fen().variant(opts.fen));

            UART_SETUP.store(true, Ordering::Relaxed);
        }
        Self { regs: uart }
    }

    pub fn new(iomux: &Iomux, uart: &'a Uart0) -> Self {
        Uart::new_with_config(UartOptions::default(), iomux, uart)
    }

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
        while self.regs.uart0_stat().read().txfe().bit_is_clear() {
            nop();
        }
    }

    pub fn read_bytes(&self, bytes: &mut [u8]) {
        for b in bytes.iter_mut() {
            while self.regs.uart0_stat().read().rxfe().bit_is_clear() {}
            let result = self.regs.uart0_rxdata().read();
            if result.brkerr().bit_is_set()
                || result.frmerr().bit_is_set()
                || result.nerr().bit_is_set()
                || result.ovrerr().bit_is_set()
                || result.parerr().bit_is_set()
            {
                panic!("UART error");
            }
            *b = result.data().bits();
        }
    }

    /// Returns whether either tx or rx is busy
    pub fn busy(&self) -> bool {
        self.regs.uart0_stat().read().busy().bit_is_set()
    }
}

impl<'a> Write for Uart<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());

        Ok(())
    }
}
