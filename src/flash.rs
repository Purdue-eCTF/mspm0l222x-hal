use cortex_m::asm::nop;
use mspm0l222x_pac::Flashctl;
use thiserror::Error;

use crate::HalError;

pub struct FlashController<'a> {
    controller: &'a Flashctl,
}

#[derive(Error, Debug)]
pub enum FlashError {
    #[error("Unknown error")]
    Unknown,
    #[error("Unaligned address (should be {0}-bit aligned)")]
    UnalignedAddress(u8),
    #[error("Unaligned data (must be {0}-byte aligned)")]
    UnalignedData(u8),
    #[error("Out-of-bounds flash address")]
    OobFlashAddress,
    #[error("Illegal flash address")]
    IllegalFlashAddress,
    #[error("Target flash address is write-protected")]
    WriteProtectedFlashAddress,
    #[error("Failed To verify")]
    FailedToVerify,
}

impl<'a> FlashController<'a> {
    pub fn new(controller: &'a Flashctl) -> Self {
        Self { controller }
    }

    fn check_addr(&self, location: u32, size: u32) -> Result<(), HalError> {
        let flash_start = 0x0;
        let flash_len = 1 << 18; // 256KiB

        // TODO: flash vs code + write protection checks
        if !(flash_start <= location && location + size < flash_start + flash_len) {
            return Err(FlashError::OobFlashAddress.into());
        }

        if location & 0b111 != 0 {
            return Err(FlashError::UnalignedAddress(3).into());
        }

        Ok(())
    }
    pub fn write_word(&self, location: u32, word: [u8; 8]) -> Result<(), HalError> {
        self.check_addr(location, 8)?;

        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().program().size().oneword());
        self.addr(location);

        // Flash is split into 64-bit "words", but we can only write 32 bits per operation, so the write is split across two registers.
        // The register byte order is the same as the system byte order, so this will leave the data unchanged
        let [a, b]: [u32; 2] = bytemuck::cast(word);

        self.controller
            .flashctl_cmddata0()
            .write(|w| unsafe { w.bits(a) });
        self.controller
            .flashctl_cmddata1()
            .write(|w| unsafe { w.bits(b) });

        self.cmd_exec();
        self.wait_done();
        self.check_error()?;

        // prevent accidental operations (suggested by manual)
        self.cmd_noop();

        // TODO: flush cache?
        // from manual:
        // Following programming of the flash memory, it is possible that there may be stale data in the processor's
        // cache and prefetch logic. Before reading locations which were programmed, it is recommended to first flush
        // the cache in the CPU subsystem.
        Ok(())
    }

    /// Write an 8-byte aligned object into flash at a given location. To verify that the
    /// write succeded, use `FlashController::verify`
    pub fn write_data<T>(&self, location: u32, data: &T) -> Result<(), HalError>
    where
        T: bytemuck::Pod,
    {
        let data_bytes = bytemuck::bytes_of(data);
        // make sure data is 8-byte aligned before writing
        let (chunks, []): (&[[u8; 8]], _) = data_bytes.as_chunks() else {
            return Err(FlashError::UnalignedData(8).into());
        };

        for (i, chunk) in chunks.iter().enumerate() {
            self.write_word(location + 8 * (i as u32), *chunk)?;
        }

        Ok(())
    }

    /// Erase a 1kb sector of flash
    pub fn erase(&self, location: u32) -> Result<(), HalError> {
        self.check_addr(location, 1 << 10)?;
        // address must be aligned to 1kb
        if location & 0x3ff != 0 {
            return Err(FlashError::UnalignedAddress(10).into());
        }
        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().erase().size().sector());

        self.addr(location);

        self.cmd_exec();
        self.wait_done();
        self.check_error()?;

        // prevent accidental operations (suggested by manual)
        self.cmd_noop();

        // TODO: flush cache?
        // from manual:
        // Following programming of the flash memory, it is possible that there may be stale data in the processor's
        // cache and prefetch logic. Before reading locations which were programmed, it is recommended to first flush
        // the cache in the CPU subsystem.

        Ok(())
    }

    /// Verify an entry in flash
    pub fn verify<T>(&self, location: u32, data: &T) -> Result<(), HalError>
    where
        T: bytemuck::Pod,
    {
        self.check_addr(location, size_of::<T>() as u32)?;

        let data_bytes = bytemuck::bytes_of(data);
        // make sure data is 8-byte aligned before verifying
        let (chunks, []): (&[[u8; 8]], _) = data_bytes.as_chunks() else {
            return Err(FlashError::UnalignedData(8).into());
        };

        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().readverify().size().eightword());
        self.addr(location);

        for chunk in chunks {
            let [a, b]: [u32; 2] = *bytemuck::from_bytes(chunk);
            self.controller
                .flashctl_cmddata0()
                .write(|w| unsafe { w.bits(a) });
            self.controller
                .flashctl_cmddata1()
                .write(|w| unsafe { w.bits(b) });

            self.cmd_exec();
            self.wait_done();
            self.check_error()?;
        }

        Ok(())
    }

    fn check_error(&self) -> Result<(), HalError> {
        let stat = self.controller.flashctl_statcmd().read();
        if stat.cmdpass().is_statfail() {
            if stat.faililladdr().bit_is_set() {
                return Err(FlashError::IllegalFlashAddress.into());
            } else if stat.failweprot().bit_is_set() {
                return Err(FlashError::WriteProtectedFlashAddress.into());
            } else if stat.failverify().bit_is_set() {
                return Err(FlashError::FailedToVerify.into());
            }

            return Err(FlashError::Unknown.into());
        }
        Ok(())
    }

    fn cmd_exec(&self) {
        self.controller
            .flashctl_cmdexec()
            .write(|w| w.val().execute());
    }

    fn cmd_noop(&self) {
        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().noop());
    }

    fn addr(&self, location: u32) {
        self.controller
            .flashctl_cmdaddr()
            .write(|w| unsafe { w.val().bits(location) });
    }

    fn wait_done(&self) {
        while self
            .controller
            .flashctl_statcmd()
            .read()
            .cmddone()
            .is_statnotdone()
        {
            nop();
        }
    }
}
