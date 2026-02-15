use core::arch::asm;

use core::mem::size_of_val;
use cortex_m::asm::nop;
use mspm0l222x_pac::Flashctl;
use once_cell::sync::OnceCell;
use thiserror::Error;

use crate::uart::uart0;
use crate::HalError;

pub const FLASH_PAGE_SIZE: usize = 1024;
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
    EraseWriteProtectedFlashAddress,
    #[error("Target flash address is write-protected")]
    WriteProtectedFlashAddress,
    #[error(
        "Program command failed because an attempt was made to program a stored 0 value to a 1"
    )]
    InvData,
    #[error("Command failed because a bank has been set to a mode other than READ")]
    InvalidMode,
    #[error("Command failed due to verify error")]
    FailVerify,
    #[error("Checked error when command was not done")]
    StatNotDone,
    #[error("Address {0} was programmed without first being erased")]
    NotBlank(u32),
    #[error("Failed with misc. error")]
    FailMisc,
}

impl FlashController {
    pub fn new(controller: Flashctl) -> Self {
        Self { controller }
    }

    pub fn init(controller: Flashctl) {
        let _ = FLASH.get_or_init(|| FlashController::new(controller));
    }

    unsafe fn write_page(
        &self,
        location: u32,
        page: &[u8; FLASH_PAGE_SIZE],
    ) -> Result<(), HalError> {
        if !self.check_blank(location) {
            return Err(FlashError::NotBlank(location).into());
        }

        let flash_start = 0x0;
        let flash_len = 1 << 18; // 256KiB

        if !(flash_start <= location && location + 8 < flash_start + flash_len) {
            return Err(FlashError::OobFlashAddress.into());
        }
        if location % (FLASH_PAGE_SIZE as u32) != 0 {
            return Err(FlashError::Unaligned(10).into());
        }

        let chunks: &[[u32; 2]; 128] = bytemuck::cast_ref(page);
        for (i, chunk) in chunks.iter().enumerate() {
            self.write_unprotect(location, size_of_val(chunk) as u32);
            self.controller
                .flashctl_cmdtype()
                .write(|w| w.command().program().size().oneword());

            // enable 8 bytes of CMDBYTEEN for programming.
            // include ECC bits
            self.controller
                .flashctl_cmdbyten()
                .write(|w| w.bits(0x0003ffff));

            self.controller
                .flashctl_cmdaddr()
                .write(|w| unsafe { w.bits(location + (i * size_of_val(chunk)) as u32) });
            self.set_cmddata(chunk);
            self.run_and_check()?;
        }

        Ok(())
    }

    unsafe fn check_blank(&self, location: u32) -> bool {
        self.controller
            .flashctl_cmdtype()
            .write(|w| w.command().blankverify().size().oneword());
        self.controller
            .flashctl_cmdaddr()
            .write(|w| unsafe { w.val().bits(location) });
        let _ = self.run_and_check();

        let stat = self.controller.flashctl_statcmd().read();
        stat.cmdpass().is_statpass() && stat.failverify().is_statnofail()
    }

    fn set_cmddata(&self, data: &[u32; 2]) {
        macro_rules! cmd {
                ($count:literal) => {
                    self.controller
                        .${concat(flashctl_cmddata, $count)}()
                        .write(|w| w.bits(data[$count]));
                };
            }
        unsafe {
            cmd!(0);
            cmd!(1); // device only supports single-=word programming
        }
    }

    pub unsafe fn partial_rewrite_page(
        &self,
        location: u32,
        offset: u32,
        data: &[u8],
    ) -> Result<(), HalError> {
        if location & 0x3ff != 0 {
            return Err(FlashError::Unaligned(10).into());
        }
        let offset = offset as usize;

        if offset + data.len() > FLASH_PAGE_SIZE {
            return Err(FlashError::OobFlashAddress.into());
        }

        let old = unsafe { *(location as *const [u8; FLASH_PAGE_SIZE]) };
        let mut new = old.clone();
        new[offset..offset + data.len()].copy_from_slice(data);

        self.rewrite_page(location, &new)?;

        Ok(())
    }

    pub unsafe fn rewrite_page(
        &self,
        location: u32,
        data: &[u8; FLASH_PAGE_SIZE],
    ) -> Result<(), HalError> {
        self.write_unprotect(location, FLASH_PAGE_SIZE as u32);
        self.erase_page(location)?; // sector erase is required to reprogram
                                    // operations set write protection bits, so unclear them before writing
        self.write_unprotect(location, FLASH_PAGE_SIZE as u32);
        self.write_page(location, data)?;
        self.write_protect(location, FLASH_PAGE_SIZE as u32);

        Ok(())
    }

    /// Erase a 1kb sector of flash
    pub unsafe fn erase_page(&self, location: u32) -> Result<(), HalError> {
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

        self.run_and_check().map_err(|e| match e {
            HalError::FlashError(FlashError::WriteProtectedFlashAddress) => {
                FlashError::EraseWriteProtectedFlashAddress.into()
            }
            e => e,
        })
    }

    fn run_and_check(&self) -> Result<(), HalError> {
        self.controller
            .flashctl_cmdexec()
            .write(|w| w.val().execute());

        while self
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

        Ok(())
    }

    fn check_error(&self) -> Result<(), HalError> {
        let stat = self.controller.flashctl_statcmd().read();

        if stat.cmddone().is_statnotdone() {
            return Err(FlashError::StatNotDone.into());
        }

        if stat.cmdpass().is_statfail() {
            if stat.failinvdata().bit_is_set() {
                return Err(FlashError::InvData.into());
            } else if stat.failverify().bit_is_set() {
                return Err(FlashError::FailVerify.into());
            } else if stat.failmode().bit_is_set() {
                return Err(FlashError::InvalidMode.into());
            } else if stat.faililladdr().bit_is_set() {
                return Err(FlashError::IllegalFlashAddress.into());
            } else if stat.failweprot().bit_is_set() {
                return Err(FlashError::WriteProtectedFlashAddress.into());
            }

            return Err(FlashError::Unknown.into());
        }

        Ok(())
    }

    pub fn write_unprotect(&self, address: u32, size: u32) {
        self.change_write_protection(address, size, true)
    }

    pub fn write_protect(&self, address: u32, size: u32) {
        self.change_write_protection(address, size, false)
    }

    // TODO: verify this
    // from SDK:
    // #define FLASHCTL_SYS_WEPROTAWIDTH 32
    // #define FLASHCTL_SYS_WEPROTBWIDTH 16
    fn change_write_protection(&self, address: u32, size: u32, writeable: bool) {
        let flash_page = FLASH_PAGE_SIZE as u32;
        let end_1kb_per = 32 * flash_page;
        let in_1kb_region = address + size <= end_1kb_per;
        if in_1kb_region {
            let start = address / flash_page;
            let end = (address + size).div_ceil(flash_page);
            let mask: u32 = (start..end).map(|i| 1 << i).sum();

            self.controller.flashctl_cmdweprota().modify(|r, w| {
                let new = if writeable {
                    r.bits() & !mask
                } else {
                    r.bits() | mask
                };
                unsafe { w.bits(new) }
            });
        }

        let in_8kb_region = address + size > end_1kb_per;
        if in_8kb_region {
            // the first 4 bits correspond to same sectors as weprotA,
            // so we can ignore everything below end_1kb
            let start = end_1kb_per / (8 * flash_page);
            let end = (address + size).div_ceil(8 * flash_page);
            let mask: u32 = (start..end).map(|i| 1 << i).sum();
            self.controller.flashctl_cmdweprotb().modify(|r, w| {
                let new = if writeable {
                    r.bits() & !mask
                } else {
                    r.bits() | mask
                };
                unsafe { w.bits(new) }
            });
        }
    }
}
