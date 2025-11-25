use core::sync::atomic::{AtomicBool, Ordering};
use mspm0l222x_pac::Trng as TrngPeriph;

use crate::{PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

static TRNG_SETUP: AtomicBool = AtomicBool::new(false);

pub struct Trng<'a> {
    trng: &'a TrngPeriph,
}

impl<'a> Trng<'a> {
    // creates a new TRNG instance and performs one-time hardware init.
    pub fn new(trng: &'a TrngPeriph) -> Self {
        let this = Self { trng };

        if !TRNG_SETUP.load(Ordering::SeqCst) {
            this.init_basic();
            TRNG_SETUP.store(true, Ordering::Relaxed);
        }

        this
    }

    // minimal TRNG initialization (power, clock divider, CTL).
    fn init_basic(&self) {
        self.power_on();
        self.configure_clkdivide();
        self.configure_ctl_norm_func();

        // wait for the command to be accepted by the state machine
        self.wait_for_cmd_done();

        // discard the first sample
        let _ = self.word();
    }

    // power up the TRNG block using the GPRCM reset + pwren sequence.
    fn power_on(&self) {
        let gprcm = self.trng.trng_gprcm(0);

        // reset the module
        gprcm.trng_rstctl().write(|w| {
            unsafe { w.bits(RSTCTL_WRITE_KEY) }
                .resetassert()
                .assert()
                .resetstkyclr()
                .clr()
        });

        // enable TRNG power
        gprcm
            .trng_pwren()
            .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());

        for _ in core::hint::black_box(0..32) {}
    }

    fn configure_clkdivide(&self) {
        self.trng
            .trng_clkdivide()

            .write(|w| unsafe { w.ratio().bits(0x3) }); 
    }

    // TRNG in NORM_FUNC mode
    fn configure_ctl_norm_func(&self) {
        self.trng.trng_ctl().write(|w| unsafe {
            w.cmd().bits(0x3)        // NORM_FUNC
             .decim_rate().bits(0x3) // decimate by 4
        });
    }

    // helper to block until the TRNG state machine completes the command
    fn wait_for_cmd_done(&self) {

        while self.trng.trng_ris().read().irq_cmd_done().bit_is_clear() {}
        self.trng.trng_iclr().write(|w| w.irq_cmd_done().set_bit());
    }

    // returns the raw 32 bit TRNG output word
    pub fn word(&self) -> u32 {
        // poll IRQ_CAPTURED_RDY and wait for generation completion
        while self.trng.trng_ris().read().irq_captured_rdy().bit_is_clear() {}
        
        // reading DATA_CAPTURE automatically clears the IRQ_CAPTURED_RDY flag
        self.trng.trng_data_capture().read().bits()
    }
}