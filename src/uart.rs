use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};
use mspm0l222x_pac::{Iomux, Uart0};

static UART_SETUP: AtomicBool = AtomicBool::new(false);

const UART_FREQUENCY: u32 = 115200;

// Taken from ti/driverlib/dl_uart.c
const UART_DIVISOR: u32 = const { ((crate::SYSOSC_FREQUENCY * 8) / UART_FREQUENCY + 1) / 2 };
const UART_IBRD: u16 = (UART_DIVISOR >> 6) as u16;
const UART_FBRD: u8 = (UART_DIVISOR & 0b111111) as u8;

pub struct Uart {
    regs: Uart0,
}
impl Uart {
    pub fn new(uart: Uart0, iomux: Iomux) -> Self {
        if !UART_SETUP.load(Ordering::SeqCst) {
            // set up IOMUX to output
            iomux
                .iomux_pincm(25 - 1)
                .write(|w| unsafe { w.pf().bits(2) });

            iomux
                .iomux_pincm(26 - 1)
                .write(|w| unsafe { w.pf().bits(2) });

            // Disable UART before configuration
            //#define LCD_RSTCTL_KEY_UNLOCK_W ((uint32_t)0xB1000000U)
            //#define LCD_PWREN_KEY_UNLOCK_W ((uint32_t)0x26000000U)
            uart.uart0_gprcm(0).uart0_rstctl().write(|w| {
                unsafe { w.bits(0xB1000000) }
                    .resetassert()
                    .assert()
                    .resetstkyclr()
                    .clr()
            });
            uart.uart0_gprcm(0)
                .uart0_pwren()
                .write(|w| unsafe { w.bits(0x26000000) }.enable().set_bit());
            // TODO: wait several cycles?
            // TODO: does black_box work here?
            for _ in core::hint::black_box(0..32) {}

            // Select clock source (BUSCLK) and divisor
            uart.uart0_clksel().write(|w| w.busclk_sel().set_bit());
            uart.uart0_clkdiv().write(|w| w.ratio().div_by_1());

            // disable UART
            uart.uart0_ctl0().write(|w| w.enable().clear_bit());
            // set all UART settings
            uart.uart0_ctl0().write(|w| {
                w
                    // .hse()
                    // .ovs16() // 16x oversampling
                    .fen()
                    .enable() // Enable FIFOs
                    .txe()
                    .enable() // Enable transmitter
                    .rxe()
                    .enable() // Enable receiver
            });

            // Set baud rate divisors
            uart.uart0_ibrd()
                .write(|w| unsafe { w.divint().bits(UART_IBRD) });
            uart.uart0_fbrd()
                .write(|w| unsafe { w.divfrac().bits(UART_FBRD) });

            // Configure line control
            uart.uart0_lcrh()
                .write(|w| w.pen().disable().wlen().databit8().stp2().disable());

            // enable UART
            uart.uart0_ctl0().write(|w| w.enable().set_bit());

            UART_SETUP.store(true, Ordering::SeqCst);
        }
        Self { regs: uart }
    }

    pub fn write_bytes(&self, bytes: &[u8]) {
        let mut bytes = bytes;
        while let ([head], tail) = bytes.split_at(1) {
            if self.regs.uart0_stat().read().txff().bit_is_clear() {
                self.regs
                    .uart0_txdata()
                    .write(|w| unsafe { w.data().bits(*head) });
                bytes = tail;
            }
        }
    }

    pub fn read_bytes(&self, bytes: &mut [u8]) {
        for b in bytes.iter_mut() {
            while self.regs.uart0_stat().read().rxfe().bit_is_clear() {}
            *b = self.regs.uart0_rxdata().read().data().bits();
        }
    }
}

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());

        Ok(())
    }
}
