use cortex_m::asm::nop;
use mspm0l222x_pac::Trng as TrngPeriph;
use once_cell::sync::OnceCell;

use crate::{PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

static TRNG: OnceCell<Trng> = OnceCell::new();

/// Initializes the trng unit, creating and returning an instance of Trng.
pub fn trng() -> &'static Trng {
    TRNG.get().expect("TRNG not yet initialized")
}

/// The Trng unit is an entropy source for true random number generation.
pub struct Trng {
    trng: TrngPeriph,
}

unsafe impl Send for Trng {}
unsafe impl Sync for Trng {}

impl Trng {
    /// Creates a new TRNG instance and performs one-time hardware init.
    fn new(trng: TrngPeriph) -> Self {
        let this = Self { trng };
        this.init_trng();

        this
    }

    /// Initiate the hardware trng unit if it isn't already initiated before retrieving the trng unit.
    pub fn init(trng: TrngPeriph) {
        let _ = TRNG.get_or_init(|| Trng::new(trng));
    }

    /// TRNG initialization (power on and configuration).
    fn init_trng(&self) {
        self.power_on();
        self.configure_clkdivide();

        // must be in NORM_FUNC before issuing commands
        self.trng.trng_ctl().write(|w| unsafe { w.cmd().bits(0x3) });
        self.wait_for_cmd_done();

        // run tests with delays to ensure stability
        self.run_startup_tests();

        // clear interrupt status that may have triggered during tests
        self.trng.trng_iclr().write(|w| w.irq_captured_rdy().set_bit());

        // enter NORM_FUNC to start data generation
        self.configure_ctl_norm_func();
        
        // wait for the command to be accepted by the state machine
        self.wait_for_cmd_done();

        // enable hardware health monitoring interrupt mask
        self.trng
            .trng_imask()
            .write(|w| w.irq_health_fail().set_bit());

        // discard the first sample after startup tests
        let _ = self.word();
    }

    /// Tests to perform on the TRNG upon startup. Panic on failure.
    fn run_startup_tests(&self) {
        // execute digital startup self-test
        self.trng.trng_iclr().write(|w| w.irq_cmd_done().set_bit().irq_cmd_fail().set_bit());

        // Execute digital startup self-test
        self.trng
            .trng_ctl()
            .write(|w| unsafe { w.cmd().bits(0x1) });

        self.wait_for_cmd_done();

        // verify all 8 digital tests passed
        if self.trng.trng_test_results().read().dig_test().bits() != 0xFF {
            panic!("TRNG Digital Test Fail");
        }

        // execute analog startup self-test
        self.trng.trng_iclr().write(|w| w.irq_cmd_done().set_bit().irq_cmd_fail().set_bit());

        self.trng
            .trng_ctl()
            .write(|w| unsafe { w.cmd().bits(0x2) });

        self.wait_for_cmd_done();

        // verify analog entropy source is functional
        if self
            .trng
            .trng_test_results()
            .read()
            .ana_test()
            .bit_is_clear()
        {
            panic!("TRNG Analog Test Fail");
        }
        // trng auto-returns to NORM_FUNC after analog test
    }

    /// Power up the TRNG block using the GPRCM reset + pwren sequence.
    fn power_on(&self) {
        let gprcm = self.trng.trng_gprcm(0);

        // assert reset
        gprcm.trng_rstctl().write(|w| {
            unsafe { w.bits(RSTCTL_WRITE_KEY) }
                .resetassert().assert()
        });

        // de-assert reset
        gprcm.trng_rstctl().write(|w| {
            unsafe { w.bits(RSTCTL_WRITE_KEY) }
                .resetassert().clear_bit()
                .resetstkyclr().clr()
        });

        // enable TRNG power
        gprcm
            .trng_pwren()
            .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());

        // wait for peripheral to initialize
    }

    /// Configures the trng's clock frequency with clkdivide. (see Reference Manual section 13.2.2)
    fn configure_clkdivide(&self) {
        self.trng
            .trng_clkdivide()
            .write(|w| unsafe { w.ratio().bits(0x3) });
    }

    /// Configures the TRNG to be in NORM_FUNC mode.
    fn configure_ctl_norm_func(&self) {
        self.trng.trng_iclr().write(|w| w.irq_cmd_done().set_bit().irq_cmd_fail().set_bit());

        self.trng.trng_ctl().write(|w| unsafe {
            w.cmd()
                .bits(0x3) // NORM_FUNC
                .decim_rate()
                .bits(0x3) // decimate by 4
        });
    }

    /// Helper function to block until the TRNG state machine completes the command.
    fn wait_for_cmd_done(&self) {
        loop {
            let ris = self.trng.trng_ris().read();

            if ris.irq_cmd_done().bit_is_set() {
                self.trng.trng_iclr().write(|w| w.irq_cmd_done().set_bit());
                break;
            }

            if ris.irq_cmd_fail().bit_is_set() {
                self.trng.trng_iclr().write(|w| w.irq_cmd_fail().set_bit());
                panic!("TRNG Command Rejected");
            }
        }
    }

    /// Returns the raw 32 bit TRNG output word. Panics if FSM is in error due to runtime health failure.
    pub fn word(&self) -> u32 {
        // verify FSM is not in error state due to runtime health fail
        if self.trng.trng_stat().read().fsm_state().bits() == 0xA {
            panic!("TRNG Health Fail");
        }

        while self
            .trng
            .trng_ris()
            .read()
            .irq_captured_rdy()
            .bit_is_clear()
        {
            nop();
        }

        // reading DATA_CAPTURE automatically clears the IRQ_CAPTURED_RDY flag
        self.trng.trng_data_capture().read().bits()
    }
}
