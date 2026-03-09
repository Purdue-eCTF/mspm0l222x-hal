use cortex_m::{asm, peripheral::MPU};
use thiserror::Error;

use crate::HalError;

// Region base address uses bits [31:5]; bits [3:0] select region when VALID is set.
const RBAR_ADDR_MASK: u32 = 0xffffffe0;
const RBAR_REGION_MASK: u32 = 0xf;
const RBAR_VALID: u32 = 1 << 4;

const RASR_EXECUTE_DISABLE: u32 = 1 << 28;
const RASR_AP_NO_ACCESS: u32 = 0 << 24;
const RASR_AP_PRIV_RW: u32 = 0b001 << 24;
const RASR_AP_PRIV_RO: u32 = 0b101 << 24;
const RASR_ENABLED: u32 = 1;

const MPU_CTRL_ENABLE: u32 = 1;
const MPU_CTRL_HFNMIENA: u32 = 1 << 1;
const MPU_CTRL_PRIVDEFENA: u32 = 1 << 2;
const MPU_MAX_REGIONS: u32 = 8;

pub struct Mpu {
    regs: MPU,
}

/// Access permissions for an MPU region.
#[derive(Debug, Clone, Copy)]
pub struct MpuPerms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

/// MPU region size encoding for RASR.SIZE (actual size is 2^(SIZE + 1)).
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum MpuRegionSize {
    Bytes32 = 0x4,
    Bytes64 = 0x5,
    Bytes128 = 0x6,
    Bytes256 = 0x7,
    Bytes512 = 0x8,
    KibiByte1 = 0x9,
    KibiByte2 = 0xA,
    KibiByte4 = 0xB,
    KibiByte8 = 0xC,
    KibiByte16 = 0xD,
    KibiByte32 = 0xE,
    KibiByte64 = 0xF,
    KibiByte128 = 0x10,
    KibiByte256 = 0x11,
    KibiByte512 = 0x12,
    MibiByte1 = 0x13,
    MibiByte2 = 0x14,
    MibiByte4 = 0x15,
    MibiByte8 = 0x16,
    MibiByte16 = 0x17,
    MibiByte32 = 0x18,
    MibiByte64 = 0x19,
    MibiByte128 = 0x1A,
    MibiByte256 = 0x1B,
    MibiByte512 = 0x1C,
    GibiByte1 = 0x1D,
    GibiByte2 = 0x1E,
    GibiByte4 = 0x1F,
}

/// Cache/shareability policy for an MPU region.
#[derive(Debug, Clone, Copy)]
pub enum MemoryCacheType {
    StronglyOrdered,
    DeviceShared,
    WriteBackUnshared,
}

impl MemoryCacheType {
    fn make_memory_type_bits(tex: u32, c: u32, b: u32, s: u32) -> u32 {
        (tex & 0b111) << 3 | (s & 1) << 2 | (c & 1) << 1 | (b & 1)
    }

    /// Convert to TEX/C/B/S encoding used by RASR.
    fn to_bits(self) -> u32 {
        match self {
            Self::StronglyOrdered => Self::make_memory_type_bits(0, 0, 0, 0),
            Self::DeviceShared => Self::make_memory_type_bits(0, 0, 1, 0),
            Self::WriteBackUnshared => Self::make_memory_type_bits(0, 1, 1, 0),
        }
    }
}

#[derive(Error, Debug)]
pub enum MpuError {
    #[error("attempted to set SRAM RW region to length of 0")]
    ZeroLength,
    #[error("attempted to set SRAM boundary to {0} (past end of SRAM)")]
    PastBoundary(u32),
    #[error("region number {0} is out of range")]
    InvalidRegion(u32),
    #[error("base address 0x{0:08x} is not 32-byte aligned")]
    UnalignedBaseAddress(u32),
}

impl Mpu {
    pub fn new(regs: MPU) -> Self {
        Self { regs }
    }

    /// Returns the number of regions implemented by the core.
    pub fn region_count(&self) -> u8 {
        ((self.regs._type.read() >> 8) & 0xff) as u8
    }

    /// Returns true when an MPU is implemented and exposes at least one region.
    ///
    /// On ARM Cortex-M, this is determined from MPU.TYPE.DREGION.
    pub fn is_available(&self) -> bool {
        self.region_count() > 0
    }

    unsafe fn set_region_inner(&mut self, rbar: u32, rasr: u32) {
        self.regs.rbar.write(rbar);
        self.regs.rasr.write(rasr);
    }

    fn construct_rbar(region_number: u32, base_address: u32) -> Result<u32, HalError> {
        if region_number >= MPU_MAX_REGIONS {
            return Err(MpuError::InvalidRegion(region_number).into());
        }
        if base_address & !RBAR_ADDR_MASK != 0 {
            return Err(MpuError::UnalignedBaseAddress(base_address).into());
        }

        Ok((base_address & RBAR_ADDR_MASK) | (region_number & RBAR_REGION_MASK) | RBAR_VALID)
    }

    fn construct_rasr(
        size: MpuRegionSize,
        disable_mask: u8,
        permissions: MpuPerms,
        cache_type: MemoryCacheType,
    ) -> u32 {
        let execute_disable = if permissions.execute {
            0
        } else {
            RASR_EXECUTE_DISABLE
        };

        let access_perms = match (permissions.read, permissions.write) {
            (false, false) => RASR_AP_NO_ACCESS,
            (true, false) => RASR_AP_PRIV_RO,
            (_, true) => RASR_AP_PRIV_RW,
        };

        execute_disable
            | access_perms
            | (cache_type.to_bits() << 16)
            | ((disable_mask as u32) << 8)
            | ((size as u32) << 1)
            | RASR_ENABLED
    }

    /// Configure one MPU region with base, size, permissions, and memory type.
    pub unsafe fn set_region(
        &mut self,
        region_number: u32,
        base_address: u32,
        region_size: MpuRegionSize,
        disable_mask: u8,
        permissions: MpuPerms,
        cache_type: MemoryCacheType,
    ) -> Result<(u32, u32), HalError> {
        let rbar = Self::construct_rbar(region_number, base_address)?;
        let rasr = Self::construct_rasr(region_size, disable_mask, permissions, cache_type);
        self.set_region_inner(rbar, rasr);
        Ok((rbar, rasr))
    }

    /// Disable one MPU region.
    pub unsafe fn clear_region(&mut self, region_number: u32) -> Result<(), HalError> {
        let rbar = Self::construct_rbar(region_number, 0)?;
        self.set_region_inner(rbar, 0);
        Ok(())
    }

    /// Enable MPU and keep default map for privileged access.
    pub unsafe fn enable(&mut self) {
        self.regs
            .ctrl
            .write(MPU_CTRL_ENABLE | MPU_CTRL_HFNMIENA | MPU_CTRL_PRIVDEFENA);
        asm::dsb();
        asm::isb();
    }

    /// Disable MPU.
    pub unsafe fn disable(&mut self) {
        self.regs.ctrl.write(0);
        asm::dsb();
        asm::isb();
    }

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
        if u32::from(addr) > crate::SRAM_SIZE {
            return Err(MpuError::PastBoundary(u32::from(addr)).into());
        }

        sysctl
            .sysctl_sramboundary()
            .write(|w| unsafe { w.addr().bits(addr) });

        Ok(())
    }
}
