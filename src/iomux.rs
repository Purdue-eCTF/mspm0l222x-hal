/// The iomux, which controls the digital IO pins.
pub struct Iomux<'a> {
    iomux: &'a mspm0l222x_pac::Iomux,
}

impl<'a> Iomux<'a> {
    /// TODO Create the iomux controller.
    pub fn new(iomux: &'a mspm0l222x_pac::Iomux) -> Self {
        Self { iomux }
    }

    /// Connect a digital pin to a peripheral pin (function); the largest valid pin is 74, and the
    /// largest valid peripheral function is 11..
    pub fn connect_pin(&self, pin: usize, function: u8) {
        assert!(pin <= 74, "Largest valid pin is 74, not {pin}");
        assert!(
            function <= 11,
            "Largest valid peripheral function is 11, not {function}"
        );

        self.iomux
            .iomux_pincm(pin)
            .write(|w| unsafe { w.pf().bits(function) }.pc().connected());
    }
}
