use mspm0l222x_pac::Trng as TrngPeriph;

pub struct Trng<'a> {
    trng: &'a TrngPeriph,
}

impl<'a> Trng<'a> {
    pub fn new(trng: &'a TrngPeriph) -> Self {
        Self { trng }
    }

    pub fn word(&self) -> u32 {
        self.trng.trng_data_capture().read().bits()
    }
}