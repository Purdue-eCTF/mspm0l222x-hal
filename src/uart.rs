use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};
use mspm0l222x_pac::Uart0;

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
    pub(crate) fn new(uart: Uart0) -> Self {
        if !UART_SETUP.load(Ordering::SeqCst) {
            // 16x oversampling
            uart.uart0_ctl0().write(|w| w.hse().ovs16());
            // TODO: check that this clock is correct
            uart.uart0_clksel().write(|w| w.busclk_sel().set_bit());

            // set baud rate
            uart.uart0_ibrd()
                .write(|w| unsafe { w.divint().bits(UART_IBRD) });
            uart.uart0_fbrd()
                .write(|w| unsafe { w.divfrac().bits(UART_FBRD) });

            // disable parity, 8-bit data, 1 stop bit
            uart.uart0_lcrh()
                .write(|w| w.pen().disable().wlen().databit8().stp2().disable());

            // enable fifo and enable peripheral
            uart.uart0_ctl0().write(|w| w.fen().set_bit());
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

pub struct UartWriter(Uart);

impl UartWriter {
    pub fn new(uart: Uart0) -> Self {
        Self(Uart::new(uart))
    }
}
impl Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_bytes(s.as_bytes());

        Ok(())
    }
}
