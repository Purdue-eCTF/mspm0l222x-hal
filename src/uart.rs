use core::fmt::{self, Write};
use cortex_m::asm::nop;
use mspm0l222x_pac::{Iomux, Uart0};
use once_cell::sync::OnceCell;

use crate::{PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

const UART_FREQUENCY: u32 = 115200;
const TX_IOMUX: usize = 25 - 1;
const RX_IOMUX: usize = 26 - 1;

static UART: OnceCell<Uart> = OnceCell::new();

pub fn uart() -> &'static Uart {
    &UART.get().expect("uart not yet initialized")
}

// Taken from ti/driverlib/dl_uart.c
const fn divisor(freq: u32) -> u32 {
    ((crate::SYSOSC_FREQUENCY * 8) / freq + 1) / 2
}

use mspm0l222x_pac::uart0::uart0_ctl0::{Ctsen, Fen, Hse, Mode, Rtsen, Rxe, Txe};
use mspm0l222x_pac::uart0::uart0_lcrh::{Pen, Stp2, Wlen};

pub struct UartOptions {
    pub hse: Hse,
    pub fen: Fen,
    pub txe: Txe,
    pub rxe: Rxe,
    pub mode: Mode,
    pub ctsen: Ctsen,
    pub rtsen: Rtsen,
    pub uart_freq: u32,
    pub pen: Pen,
    pub wlen: Wlen,
    pub stp2: Stp2,
}

impl Default for UartOptions {
    fn default() -> Self {
        Self {
            hse: mspm0l222x_pac::uart0::uart0_ctl0::Hse::Ovs16,
            fen: Fen::Enable,
            txe: Txe::Disable,
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

pub struct Uart {
    regs: Uart0,
}
// TODO: is this fine?
unsafe impl Send for Uart {}
unsafe impl Sync for Uart {}

impl Uart {
    pub fn init(iomux: &Iomux, uart: Uart0) {
        let _ = UART.get_or_init(|| Uart::new(iomux, uart));
    }

    fn new_with_config(opts: UartOptions, iomux: &Iomux, uart: Uart0) -> Self {
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

        // disable UART
        uart.uart0_ctl0().write(|w| w.enable().clear_bit());

        // Select clock source (BUSCLK) and divisor
        uart.uart0_clksel().write(|w| w.busclk_sel().enable());
        uart.uart0_clkdiv().write(|w| w.ratio().div_by_1());

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
                .txd_out_en()
                .enable()
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
        uart.uart0_ctl0().modify(|_, w| w.enable().enable());

        Self { regs: uart }
    }

    fn new(iomux: &Iomux, uart: Uart0) -> Self {
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

    pub fn set_tx(&self, state: bool) {
        self.regs.uart0_ctl0().modify(|_, w| w.txd_out().bit(state));
    }
}

impl<'a> Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());

        Ok(())
    }
}
