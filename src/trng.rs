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
    /// creates a new TRNG instance and performs one-time hardware init.
    fn new(trng: TrngPeriph) -> Self {
        let this = Self { trng };

        // reset and power on TRNG
        let gprcm = this.trng.trng_gprcm(0);
        gprcm.trng_rstctl().write(|w| {
            unsafe { w.bits(RSTCTL_WRITE_KEY) }
                .resetassert()
                .assert()
                .resetstkyclr()
                .clr()
        });

        gprcm
            .trng_pwren()
            .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());

        // wait for peripheral to initialize
        for _ in core::hint::black_box(0..32) {
            nop();
        }

        // TODO: why this particular division?
        this.trng.trng_clkdivide().write(|w| w.ratio().div_by_4());

        this.trng.trng_ctl().write(|w| w.cmd().norm_func());
        this.wait_for_cmd_done();

        this.run_startup_tests();

        // clear interrupt status that may have triggered during tests
        this.trng.trng_iclr().write(|w| {
            w.irq_captured_rdy()
                .set_bit()
                .irq_cmd_done()
                .set_bit()
                .irq_cmd_fail()
                .set_bit()
        });

        // Decimate by 8 to increase entropy per sample
        // From docs: When the DECIM_RATE field is changed, a NORM_FUNC command must be re-sent to the TRNG for
        // the new rate to take effect
        this.trng
            .trng_ctl()
            .write(|w| unsafe { w.cmd().norm_func().decim_rate().bits(0x7) });

        // wait for the command to be accepted by the state machine
        this.wait_for_cmd_done();

        // enable hardware health monitoring interrupt mask
        this.trng
            .trng_imask()
            .write(|w| w.irq_health_fail().set_bit());

        // discard the first sample after startup tests
        let _ = this.word();

        this
    }

    pub fn init(trng: TrngPeriph) {
        let _ = TRNG.get_or_init(|| Trng::new(trng));
    }

    fn run_startup_tests(&self) {
        self.trng
            .trng_iclr()
            .write(|w| w.irq_cmd_done().set_bit().irq_cmd_fail().set_bit());
        self.trng.trng_ctl().modify(|_, w| w.cmd().pwrup_dig());

        self.wait_for_cmd_done();

        // verify all 8 digital tests passed
        if self.trng.trng_test_results().read().dig_test().bits() != 0xFF {
            panic!("TRNG Digital Test Fail");
        }

        // execute analog startup self-test
        self.trng
            .trng_iclr()
            .write(|w| w.irq_cmd_done().set_bit().irq_cmd_fail().set_bit());

        self.trng.trng_ctl().modify(|_, w| w.cmd().pwrup_ana());

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

    // helper to block until the TRNG state machine completes the command
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

    // returns the raw 32 bit TRNG output word
    pub fn word(&self) -> u32 {
        // verify FSM is not in error state due to runtime health fail
        // TODO: from docs:
        // Current state of the front end FSM (behind a clock domain crossing).
        // 2 reads are REQUIRED as there is a chance of metastability when
        // reading this

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
