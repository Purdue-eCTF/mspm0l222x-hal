use thiserror::Error;

use crate::HalError;
pub struct Mpu;

#[derive(Error, Debug)]
pub enum MpuError {
    #[error("attempted to set SRAM RW region to length of 0")]
    ZeroLength,
    #[error("attempted to set SRAM boundary to {0} (past end of SRAM)")]
    PastBoundary(u32),
}

impl Mpu {
    /// Set the SRAM RW/RX boundary. Addresses below the input will be read/write,
    /// and addresses at or above the input will be read/execute. From the manual:
    ///
    /// Buffer overflows are a common source of exploits wherein, for example, a corrupt return address can cause
    /// execution to jump to malicious code. In order to mitigate such exploits, an SRAM code protection feature is
    /// available, wherein the SRAM can be partitioned into two regions:
    /// - Region 1: Read-Write (RW)
    /// - Region 2: Read-Execute (RX)
    ///
    /// This is set up by configuring the SYSCTL.SOCLOCK.SRAMBOUNDARY register with an address A such that:
    /// - Addresses >= A will be permitted for read-execute and not for writes
    /// - Addresses < A will be permitted for read-write and not for execution (instruction fetch)
    pub fn sram_rw_boundary(sysctl: &mspm0l222x_pac::Sysctl, addr: u16) -> Result<(), HalError> {
        if addr == 0 {
            return Err(MpuError::ZeroLength.into());
        }
        if u32::from(addr) >= crate::SRAM_SIZE {
            return Err(MpuError::PastBoundary(u32::from(addr)).into());
        }

        sysctl
            .sysctl_sramboundary()
            .write(|w| unsafe { w.addr().bits(addr) });

        Ok(())
    }
}
