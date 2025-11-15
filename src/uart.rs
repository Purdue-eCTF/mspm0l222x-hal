use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};
use mspm0l222x_pac::{Iomux, Uart0};

static UART_SETUP: AtomicBool = AtomicBool::new(false);

const UART_FREQUENCY: u32 = 115200;
const UART_OVERSAMPLE: u32 = 16;

// BRD = UART Clock / (Oversampling x Baud rate)
const UART_IBRD: u16 = const {
    let integer_part = crate::SYSOSC_FREQUENCY / (UART_OVERSAMPLE * UART_FREQUENCY);
    assert!(integer_part <= u16::MAX as u32);
    integer_part as u16
};
const UART_FBRD: u8 = const {
    // needs floating point math, calculated as:
    // round((clk / (oversample * rate) % 1) * 64)
    assert!(UART_FREQUENCY == 115200);
    23
};

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
            uart.uart0_gprcm(0).uart0_rstctl().write(|w| unsafe {
                w.bits(0xB1000000)
                    .resetassert()
                    .assert()
                    .resetstkyclr()
                    .clr()
            });
            uart.uart0_gprcm(0)
                .uart0_pwren()
                .write(|w| unsafe { w.bits(0x26000000).enable().set_bit() });

            // TODO: wait several cycles?
            // Select clock source (BUSCLK)
            uart.uart0_clksel().write(|w| w.busclk_sel().set_bit());

            // Set baud rate divisors
            uart.uart0_ibrd()
                .write(|w| unsafe { w.divint().bits(UART_IBRD) });
            uart.uart0_fbrd()
                .write(|w| unsafe { w.divfrac().bits(UART_FBRD) });

            // disable UART
            uart.uart0_ctl0().write(|w| w.enable().clear_bit());
            // set all UART settings
            uart.uart0_ctl0().write(|w| {
                w.hse()
                    .ovs16() // 16x oversampling
                    .fen()
                    .set_bit() // Enable FIFOs
                    .txe()
                    .set_bit() // Enable transmitter
                    .rxe()
                    .set_bit() // Enable receiver
            });
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
