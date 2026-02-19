use mspm0l222x_pac::{Gpioa, Gpiob};
use once_cell::sync::OnceCell;

use crate::{
    iomux::{Iomux, PullMode},
    PWREN_WRITE_KEY, RSTCTL_WRITE_KEY,
};

pub struct Led {
    gpioa: Gpioa,
    gpiob: Gpiob,
}

// TODO: is this fine?
unsafe impl Send for Led {}
unsafe impl Sync for Led {}

pub enum LedColor {
    Red,
    Blue,
    Green,
}

pub enum GpioBank<'a> {
    GpioA(&'a Gpioa),
    GpioB(&'a Gpiob),
}

static LED: OnceCell<Led> = OnceCell::new();

pub fn led() -> &'static Led {
    LED.get().expect("LEDs not yet initialized")
}

pub fn enable_gpio(bank: &mut GpioBank, pin: u8) {
    assert!(pin < 32, "pins >= 32 not implemented");
    match bank {
        GpioBank::GpioA(gpioa) => {
            gpioa.gpioa_gprcm(0).gpioa_rstctl().write(|w| {
                unsafe { w.bits(RSTCTL_WRITE_KEY) }
                    .resetassert()
                    .assert()
                    .resetstkyclr()
                    .clr()
            });
            gpioa
                .gpioa_gprcm(0)
                .gpioa_pwren()
                .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());
            // wait for GPIO to turn on
            for _ in core::hint::black_box(0..32) {}
            gpioa
                .gpioa_doutclr31_0()
                .write(|w| unsafe { w.bits(1 << pin) });
            gpioa
                .gpioa_doeset31_0()
                .write(|w| unsafe { w.bits(1 << pin) });
        }
        GpioBank::GpioB(gpiob) => {
            gpiob.gpiob_gprcm(0).gpiob_rstctl().write(|w| {
                unsafe { w.bits(RSTCTL_WRITE_KEY) }
                    .resetassert()
                    .assert()
                    .resetstkyclr()
                    .clr()
            });
            gpiob
                .gpiob_gprcm(0)
                .gpiob_pwren()
                .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());
            // wait for GPIO to turn on
            for _ in core::hint::black_box(0..32) {}
            gpiob
                .gpiob_doutclr31_0()
                .write(|w| unsafe { w.bits(1 << pin) });
            gpiob
                .gpiob_doeset31_0()
                .write(|w| unsafe { w.bits(1 << pin) });
        }
    }
}

impl Led {
    pub fn init(iomux: &Iomux, gpioa: Gpioa, gpiob: Gpiob) {
        let _ = LED.get_or_init(|| Led::new(iomux, gpioa, gpiob));
    }

    pub fn new(iomux: &Iomux, gpioa: Gpioa, gpiob: Gpiob) -> Self {
        // #define GPIO_RSTCTL_KEY_UNLOCK_W ((uint32_t)0xB1000000U)
        // #define GPIO_PWREN_KEY_UNLOCK_W ((uint32_t)0x26000000U)

        // enable IOMUX output and set function to 1 (GPIO)
        // PINCM42 -> PA16
        // PINCM30 -> PB9
        // PINCM31 -> PB10

        iomux.connect_pin(42, 1, PullMode::None);
        iomux.connect_pin(30, 1, PullMode::None);
        iomux.connect_pin(31, 1, PullMode::None);

        enable_gpio(&mut GpioBank::GpioA(&gpioa), 16);
        enable_gpio(&mut GpioBank::GpioB(&gpiob), 10);
        enable_gpio(&mut GpioBank::GpioB(&gpiob), 9);

        Self { gpioa, gpiob }
    }
    pub fn set(&self, color: LedColor) {
        match color {
            LedColor::Blue => self.gpioa.gpioa_dout19_16().write(|w| w.dio16().one()),
            LedColor::Red => self.gpiob.gpiob_dout11_8().write(|w| w.dio10().one()),
            LedColor::Green => self.gpiob.gpiob_dout11_8().write(|w| w.dio9().one()),
        };
    }

    pub fn clear(&self, color: LedColor) {
        // from LP-MSPM0L2228 docs:
        // PA16 -> blue
        // PB10 -> red
        // PB09 -> green
        match color {
            LedColor::Blue => self.gpioa.gpioa_doutclr31_0().write(|w| w.dio16().clr()),
            LedColor::Red => self.gpiob.gpiob_doutclr31_0().write(|w| w.dio10().clr()),
            LedColor::Green => self.gpiob.gpiob_doutclr31_0().write(|w| w.dio9().clr()),
        };
    }

    pub fn write(&self, color: LedColor, state: bool) {
        if state {
            self.set(color);
        } else {
            self.clear(color);
        }
    }

    pub fn toggle(&self, color: LedColor) {
        match color {
            LedColor::Blue => self.gpioa.gpioa_douttgl31_0().write(|w| w.dio16().toggle()),
            LedColor::Red => self.gpiob.gpiob_douttgl31_0().write(|w| w.dio10().toggle()),
            LedColor::Green => self.gpiob.gpiob_douttgl31_0().write(|w| w.dio9().toggle()),
        };
    }
}
