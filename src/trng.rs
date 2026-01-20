use cortex_m::asm::nop;
use mspm0l222x_pac::Trng as TrngPeriph;
use once_cell::sync::OnceCell;

use crate::{PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

static TRNG: OnceCell<Trng> = OnceCell::new();

pub fn trng() -> &'static Trng {
    TRNG.get().expect("TRNG not yet initialized")
}

pub struct Trng {
    trng: TrngPeriph,
}

unsafe impl Send for Trng {}
unsafe impl Sync for Trng {}

impl Trng {
    // creates a new TRNG instance and performs one-time hardware init.
    fn new(trng: TrngPeriph) -> Self {
        let this = Self { trng };
        this.init_trng();

        this
    }

    pub fn init(trng: TrngPeriph) {
        let _ = TRNG.get_or_init(|| Trng::new(trng));
    }

    // TRNG initialization
    fn init_trng(&self) {
        self.power_on();
        self.configure_clkdivide();

        self.run_startup_tests();

        // enter NORM_FUNC to start data generation
        self.configure_ctl_norm_func();
        // wait for the command to be accepted by the state machine
        self.wait_for_cmd_done();

        // enable hardware health monitoring interrupt mask
        self.trng.trng_imask().write(|w| w.irq_health_fail().set_bit());

        // discard the first sample after startup tests
        let _ = self.word();
}

    fn run_startup_tests(&self) {
    // execute digital startup self-test
    self.trng.trng_ctl().modify(|_, w| unsafe { w.cmd().bits(0x1) });
    self.wait_for_cmd_done();
    
    // verify all 8 digital tests passed
    if self.trng.trng_test_results().read().dig_test().bits() != 0xFF {
        panic!("TRNG Digital Test Fail");
    }

    // execute analog startup self-test
    self.trng.trng_ctl().modify(|_, w| unsafe { w.cmd().bits(0x2) });
    self.wait_for_cmd_done();
    
    // verify analog entropy source is functional
    if self.trng.trng_test_results().read().ana_test().bit_is_clear() {
        panic!("TRNG Analog Test Fail");
    }
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

        // wait for peripheral to initialize
        for _ in core::hint::black_box(0..32) {
            nop();
        }
    }

    fn configure_clkdivide(&self) {
        self.trng
            .trng_clkdivide()
            .write(|w| unsafe { w.ratio().bits(0x3) });
    }

    // TRNG in NORM_FUNC mode
    fn configure_ctl_norm_func(&self) {
        self.trng.trng_ctl().write(|w| unsafe {
            w.cmd()
                .bits(0x3) // NORM_FUNC
                .decim_rate()
                .bits(0x3) // decimate by 4
        });
    }

    // helper to block until the TRNG state machine completes the command
    fn wait_for_cmd_done(&self) {
        while self.trng.trng_ris().read().irq_cmd_done().bit_is_clear() {}
        self.trng.trng_iclr().write(|w| w.irq_cmd_done().set_bit());
    }

    // returns the raw 32 bit TRNG output word
    pub fn word(&self) -> u32 {
        // verify FSM is not in error state due to runtime health fail
        if self.trng.trng_stat().read().fsm_state().bits() == 0xA {
            panic!("TRNG Health Fail");
        }

        while self.trng.trng_ris().read().irq_captured_rdy().bit_is_clear() {
            nop();
        }

        // reading DATA_CAPTURE automatically clears the IRQ_CAPTURED_RDY flag
        self.trng.trng_data_capture().read().bits()
    }
    
}
