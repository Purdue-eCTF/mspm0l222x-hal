use crate::HalError;
use cortex_m::asm::nop;
use mspm0l222x_pac::Flashctl;

pub struct FlashController {
    controller: Flashctl,
}

impl FlashController {
    pub fn new(controller: Flashctl) -> Self {
        Self { controller }
    }

    pub fn write_word(&self, location: u32, word: [u8; 8]) -> Result<(), HalError> {
        let flash_start = 0x0;
        let flash_len = 1 << 18; // 256KiB

        // TODO: flash vs code + write protection checks
        if !(flash_start <= location && location + 8 < flash_start + flash_len) {
            return Err(HalError);
        }
        if location & 0b111 != 0 {
            return Err(HalError);
        }
        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().program().size().oneword());
        self.controller
            .flashctl_cmdaddr()
            .write(|w| unsafe { w.val().bits(location) });

        // Flash is split into 64-bit "words", but we can only write 32 bits per operation, so the write is split across two registers.
        // The register byte order is the same as the system byte order, so this will leave the data unchanged
        let [a, b]: [u32; 2] = bytemuck::cast(word);

        self.controller
            .flashctl_cmddata0()
            .write(|w| unsafe { w.bits(a) });
        self.controller
            .flashctl_cmddata1()
            .write(|w| unsafe { w.bits(b) });

        self.controller
            .flashctl_cmdexec()
            .write(|w| w.val().execute());

        while !self
            .controller
            .flashctl_statcmd()
            .read()
            .cmddone()
            .is_statnotdone()
        {
            nop();
        }
        if self
            .controller
            .flashctl_statcmd()
            .read()
            .cmdpass()
            .is_statfail()
        {
            return Err(HalError); // TODO: determine error type and return
        }

        // prevent accidental operations (suggested by manual)
        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().noop());

        // TODO: flush cache?
        // from manual:
        // Following programming of the flash memory, it is possible that there may be stale data in the processor's
        // cache and prefetch logic. Before reading locations which were programmed, it is recommended to first flush
        // the cache in the CPU subsystem.
        Ok(())
    }

    pub fn write_data(&self, location: u32, data: &[u8]) -> Result<(), HalError> {
        // TODO: should this only accept exact-sized chunks?
        let (chunks, rem): (&[[u8; 8]], &[u8]) = data.as_chunks();

        for (i, chunk) in chunks.iter().enumerate() {
            self.write_word(location + 8 * (i as u32), *chunk)?;
        }

        if !rem.is_empty() {
            // pad remaining data with zero bytes before writing
            let mut last = [0u8; 8];
            last[..rem.len()].copy_from_slice(rem);
            self.write_word(location + (chunks.len() as u32) * 8, last)?;
        }

        Ok(())
    }

    /// Erase a 1kb sector of flash
    pub fn erase(&self, location: u32) -> Result<(), HalError> {
        // TODO: location checks

        // address must be aligned to 1kb
        if location & 0x3ff != 0 {
            return Err(HalError);
        }
        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().erase().size().sector());

        self.controller
            .flashctl_cmdaddr()
            .write(|w| unsafe { w.val().bits(location) });
        self.controller
            .flashctl_cmdexec()
            .write(|w| w.val().execute());

        while !self
            .controller
            .flashctl_statcmd()
            .read()
            .cmddone()
            .is_statnotdone()
        {
            nop();
        }
        if self
            .controller
            .flashctl_statcmd()
            .read()
            .cmdpass()
            .is_statfail()
        {
            return Err(HalError); // TODO: determine error type and return
        }

        // prevent accidental operations (suggested by manual)
        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().noop());

        // TODO: flush cache?
        // from manual:
        // Following programming of the flash memory, it is possible that there may be stale data in the processor's
        // cache and prefetch logic. Before reading locations which were programmed, it is recommended to first flush
        // the cache in the CPU subsystem.

        Ok(())
    }
}
