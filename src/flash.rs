use cortex_m::asm::nop;
use mspm0l222x_pac::Flashctl;
use once_cell::sync::OnceCell;
use thiserror::Error;

use crate::HalError;

const FLASH_PAGE_SIZE: usize = 1024;
static FLASH: OnceCell<FlashController> = OnceCell::new();

/// Global instance of flash controller
pub fn flash() -> &'static FlashController {
    FLASH.get().expect("uart0 not yet initialized")
}

pub struct FlashController {
    controller: Flashctl,
}

unsafe impl Sync for FlashController {}

#[derive(Error, Debug)]
pub enum FlashError {
    #[error("Unknown error")]
    Unknown,
    #[error("Unaligned address (should be {0}-bit aligned)")]
    Unaligned(u8),
    #[error("Out-of-bounds flash address")]
    OobFlashAddress,
    #[error("Illegal flash address")]
    IllegalFlashAddress,
    #[error("Target flash address is write-protected")]
    WriteProtectedFlashAddress,
}

impl FlashController {
    pub fn new(controller: Flashctl) -> Self {
        Self { controller }
    }

    pub fn init(controller: Flashctl) {
        let _ = FLASH.get_or_init(|| FlashController::new(controller));
    }

    pub unsafe fn write_page(
        &self,
        location: u32,
        page: &[u8; FLASH_PAGE_SIZE],
    ) -> Result<(), HalError> {
        let flash_start = 0x0;
        let flash_len = 1 << 18; // 256KiB

        // TODO: flash vs code + write protection checks
        if !(flash_start <= location && location + 8 < flash_start + flash_len) {
            return Err(FlashError::OobFlashAddress.into());
        }
        if location % 1024 != 0 {
            return Err(FlashError::Unaligned(10).into());
        }

        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().program().size().oneword());
        self.controller
            .flashctl_cmdaddr()
            .write(|w| unsafe { w.val().bits(location) });

        let chunks: &[u32; 32] = bytemuck::cast_ref(page);

        for chunk in chunks {
            // TODO: this seems wildly unsafe but fine in practice
            unsafe {
                core::ptr::copy_nonoverlapping(
                    chunk as *const u32,
                    self.controller.flashctl_cmddata0().as_ptr(),
                    core::mem::size_of_val(chunk),
                );
            }
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
            self.check_error()?;

            self.controller
                .flashctl_cmdtype()
                .write(|w| w.command().noop());
        }

        Ok(())
    }
    pub unsafe fn write_word(&self, location: u32, word: [u8; 8]) -> Result<(), HalError> {
        let flash_start = 0x0;
        let flash_len = 1 << 18; // 256KiB

        // TODO: flash vs code + write protection checks
        if !(flash_start <= location && location + 8 < flash_start + flash_len) {
            return Err(FlashError::OobFlashAddress.into());
        }
        if location & 0b111 != 0 {
            return Err(FlashError::Unaligned(3).into());
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

        self.check_error()?;

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

    pub unsafe fn write_data<T>(&self, location: u32, data: &T) -> Result<(), HalError>
    where
        T: bytemuck::Pod,
    {
        let data_bytes = bytemuck::bytes_of(data);
        let (chunks, rem): (&[[u8; 8]], &[u8]) = data_bytes.as_chunks();

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
    pub unsafe fn erase(&self, location: u32) -> Result<(), HalError> {
        // address must be aligned to 1kb
        if location & 0x3ff != 0 {
            return Err(FlashError::Unaligned(10).into());
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

        self.check_error()?;

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

    fn check_error(&self) -> Result<(), HalError> {
        let stat = self.controller.flashctl_statcmd().read();
        if stat.cmdpass().is_statfail() {
            if stat.faililladdr().bit_is_set() {
                return Err(FlashError::IllegalFlashAddress.into());
            } else if stat.failweprot().bit_is_set() {
                return Err(FlashError::WriteProtectedFlashAddress.into());
            }

            return Err(FlashError::Unknown.into());
        }
        Ok(())
    }

    // TODO: add verify command
}
