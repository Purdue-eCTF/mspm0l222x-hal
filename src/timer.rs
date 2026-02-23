use mspm0l222x_pac::SYST;

pub struct SysTickConfig;
impl SysTickConfig {
    pub fn enable_with_reload(syst: &SYST, reload: u32) {
        unsafe { syst.rvr.write(reload) };
        // enable: bit 0, tickint: bit 1, clksource: bit 2
        unsafe {
            syst.csr.write(0b111);
        }
    }

    pub fn disable(syst: &SYST) {
        // clear enable bit and leave others as they are
        unsafe { syst.csr.write(syst.csr.read() & !0b1) };
    }
}
