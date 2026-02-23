pub struct Iomux<'a> {
    iomux: &'a mspm0l222x_pac::Iomux,
}

#[derive(PartialEq)]
pub enum PullMode {
    None,
    Up,
    Down,
}

#[derive(PartialEq)]
pub enum InOut {
    Input,
    Output,
}

impl<'a> Iomux<'a> {
    pub fn new(iomux: &'a mspm0l222x_pac::Iomux) -> Self {
        Self { iomux }
    }

    pub fn connect_pin(&self, pin: usize, function: u8, in_mode: InOut, pull_mode: PullMode) {
        assert!(
            (1..=74).contains(&pin),
            "valid pins are 1 <= n <= 74, not {pin}"
        );
        assert!(
            function <= 11,
            "Largest valid peripheral function is 11, not {function}"
        );

        self.iomux.iomux_pincm(pin - 1).write(|w| {
            unsafe { w.pf().bits(function) }
                .pc()
                .connected()
                .inena()
                .bit(in_mode == InOut::Input)
                .pipu()
                .bit(pull_mode == PullMode::Up)
                .pipd()
                .bit(pull_mode == PullMode::Down)
        });
    }
}
