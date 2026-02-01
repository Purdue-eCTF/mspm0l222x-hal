use cortex_m::asm::nop;
use mspm0l222x_pac::Flashctl;
use thiserror::Error;

use crate::HalError;

/// The controller for the NVM flash, used to store executable code and data.
pub struct FlashController {
    controller: Flashctl,
}

/// The error that is returned during failure of an operation with flash, often to do with the
/// address used being invalid in some way.
#[derive(Error, Debug)]
pub enum FlashError {
    /// The error is unknown...
    #[error("Unknown error")]
    Unknown,
    /// The address is incorrectly aligned; returns the correct alignment value.
    #[error("Unaligned address (should be {0}-bit aligned)")]
    Unaligned(u8),
    /// The flash address is out of bounds.
    #[error("Out-of-bounds flash address")]
    OobFlashAddress,
    /// The flash address is illegal.
    #[error("Illegal flash address")]
    IllegalFlashAddress,
    /// The flash address is write-protected.
    #[error("Target flash address is write-protected")]
    WriteProtectedFlashAddress,
}

impl FlashController {
    /// Creates a new flash instance.
    pub fn new(controller: Flashctl) -> Self {
        Self { controller }
    }

    /// Writes a given word to the flash at a given location value and returns the Result status.
    pub fn write_word(&self, location: u32, word: [u8; 8]) -> Result<(), HalError> {
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

        // delay during writing
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

        // prevent accidental operations (suggested by manual 6.3.2)
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

    /// Writes given data to the flash at a given location value and returns the Result status.
    pub fn write_data<T>(&self, location: u32, data: &T) -> Result<(), HalError>
    where
        T: bytemuck::Pod,
    {
        // TODO: add reset protection?
        // TODO: should this only accept exact-sized chunks?
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

        // TODO: verify that flash write succeeded

        Ok(())
    }

    /// Erase a 1kb sector of flash
    pub fn erase(&self, location: u32) -> Result<(), HalError> {
        // TODO: location checks

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

        // delay during writing
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

    /// Read flash command status register to check if there has been a error and returns the error
    /// if necessary.
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
