use mspm0l222x_pac::{Gpioa, Gpiob, Iomux};

pub struct Led {
    gpioa: Gpioa,
    gpiob: Gpiob,
}

pub enum LedColor {
    Red,
    Blue,
    Green,
}

impl Led {
    pub fn new(iomux: &mut Iomux, gpioa: Gpioa, gpiob: Gpiob) -> Self {
        let res = Self { gpioa, gpiob };

        // #define GPIO_RSTCTL_KEY_UNLOCK_W ((uint32_t)0xB1000000U)
        // #define GPIO_PWREN_KEY_UNLOCK_W ((uint32_t)0x26000000U)
        // power on GPIO bank A and B
        res.gpioa.gpioa_gprcm(0).gpioa_rstctl().write(|w| {
            unsafe { w.bits(0xB1000000) }
                .resetassert()
                .assert()
                .resetstkyclr()
                .clr()
        });
        res.gpiob.gpiob_gprcm(0).gpiob_rstctl().write(|w| {
            unsafe { w.bits(0xB1000000) }
                .resetassert()
                .assert()
                .resetstkyclr()
                .clr()
        });

        res.gpioa
            .gpioa_gprcm(0)
            .gpioa_pwren()
            .write(|w| unsafe { w.bits(0x26000000) }.enable().set_bit());
        res.gpiob
            .gpiob_gprcm(0)
            .gpiob_pwren()
            .write(|w| unsafe { w.bits(0x26000000) }.enable().set_bit());
        // delay while GPIOs power on
        // TODO: how many cycles to delay?
        for _ in core::hint::black_box(0..32) {}

        // enable IOMUX output and set function to 1 (GPIO)
        // PINCM42 -> PA16
        // PINCM30 -> PB9
        // PINCM31 -> PB10

        // TODO: offset by 1 or no? pa0 maps to pincm1 so probably yes?
        iomux
            .iomux_pincm(42 - 1)
            .write(|w| unsafe { w.pc().set_bit().pf().bits(1) });
        iomux
            .iomux_pincm(30 - 1)
            .write(|w| unsafe { w.pc().set_bit().pf().bits(1) });
        iomux
            .iomux_pincm(31 - 1)
            .write(|w| unsafe { w.pc().set_bit().pf().bits(1) });

        // clear pins
        res.gpioa.gpioa_doutclr31_0().write(|w| w.dio16().clr());
        res.gpiob.gpiob_doutclr31_0().write(|w| w.dio10().clr());
        res.gpiob.gpiob_doutclr31_0().write(|w| w.dio9().clr());

        // enable output
        res.gpioa.gpioa_doeset31_0().write(|w| w.dio16().set_());
        res.gpiob.gpiob_doeset31_0().write(|w| w.dio10().set_());
        res.gpiob.gpiob_doeset31_0().write(|w| w.dio9().set_());

        res
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
