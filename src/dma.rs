use core::hint::black_box;
use core::marker::PhantomData;

use cortex_m::asm::nop;
use once_cell::sync::OnceCell;

/// Number of DMA channels available
pub const DMA_CHANNEL_COUNT: usize = 7;

static DMA: OnceCell<DmaController> = OnceCell::new();

pub type Dma = DmaController;

/// Initializes the DMA
pub fn init(dma: mspm0l222x_pac::Dma) {
    let _ = DMA.get_or_init(|| DmaController::new(dma));
}

/// Access the global DMA instance
pub fn dma() -> &'static DmaController {
    DMA.get().expect("DMA not yet initialized")
}

pub struct DmaController {
    pub(crate) regs: mspm0l222x_pac::Dma,
}

unsafe impl Sync for DmaController {}

impl DmaController {
    fn new(dma: mspm0l222x_pac::Dma) -> Self {
        dma.dma_cpu_int(0)
            .dma_cpu_int_iclr()
            .write(|w| unsafe { w.bits((1u32 << DMA_CHANNEL_COUNT) - 1) });

        for _ in black_box(0..32) {
            nop();
        }

        Self { regs: dma }
    }
}

/// DMA Trigger Sources
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TriggerSource {
    Software = 0,
    GenericSubscriber0 = 1,
    GenericSubscriber1 = 2,
    AesPublisher1 = 3,
    AesPublisher1Alt = 4,
    I2c0Publisher1 = 5,
    I2c0Publisher2 = 6,
    I2c1Publisher1 = 7,
    I2c1Publisher2 = 8,
    I2c2Publisher1 = 9,
    I2c2Publisher2 = 10,
    Spi0Publisher1 = 11,
    Spi0Publisher2 = 12,
    Spi1Publisher1 = 13,
    Spi1Publisher2 = 14,
    Uart0Publisher1 = 15,
    Uart0Publisher2 = 16,
    Uart1Publisher1 = 17,
    Uart1Publisher2 = 18,
    Uart2Publisher1 = 19,
    Uart2Publisher2 = 20,
    Uart3Publisher1 = 21,
    Uart3Publisher2 = 22,
    Uart4Publisher1 = 23,
    Uart4Publisher2 = 24,
    Adc0Publisher2 = 25,
}

/// DMA Transfer Modes
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TransferMode {
    Single = 0,
    Block = 1,
    RepeatedSingle = 2,
    RepeatedBlock = 3,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DataWidth {
    Byte = 0,     // 8-bit
    HalfWord = 1, // 16-bit
    Word = 2,     // 32-bit
    LongWord = 3, // 64-bit
}

/// DMA address increment mode
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AddressIncrement {
    Unchanged = 0, // +0
    Decrement = 2, // -1 * width
    Increment = 3, // +1 * width
    Stride2 = 8,   // +2 * width
    Stride3 = 9,   // +3 * width
    Stride4 = 10,  // +4 * width
    Stride5 = 11,  // +5 * width
    Stride6 = 12,  // +6 * width
    Stride7 = 13,  // +7 * width
    Stride8 = 14,  // +8 * width
    Stride9 = 15,  // +9 * width
}

pub struct DmaChannel<const N: usize>(PhantomData<()>);

impl<const N: usize> DmaChannel<N> {
    /// Construct channel handle
    pub const fn new() -> Self {
        Self(PhantomData)
    }

    #[inline(always)]
    fn check_channel() {
        assert!(N < DMA_CHANNEL_COUNT, "DMA channel out of range");
    }

    /// Configures a DMA transfer
    pub unsafe fn configure(
        &self,
        src_addr: u32,
        dst_addr: u32,
        transfer_count: u16,
        mode: TransferMode,
        width: DataWidth,
        src_incr: AddressIncrement,
        dst_incr: AddressIncrement,
    ) {
        Self::check_channel();
        let dma = dma();

        let chan = dma.regs.dma_dmachan(N);

        // Disable channel before reconfiguration
        chan.dma_dmactl().modify(|_, w| w.dmaen().clear_bit());

        // Source and destination addresses
        chan.dma_dmasa()
            .write(|w| unsafe { w.addr().bits(src_addr) });
        chan.dma_dmada()
            .write(|w| unsafe { w.addr().bits(dst_addr) });

        // Transfer size in number of transfers
        chan.dma_dmasz()
            .write(|w| unsafe { w.size().bits(transfer_count) });

        let width = width as u8;

        // Control register
        chan.dma_dmactl().write(|w| unsafe {
            w.dmatm()
                .bits(mode as u8)
                .dmasrcwdth()
                .bits(width)
                .dmadstwdth()
                .bits(width)
                .dmasrcincr()
                .bits(src_incr as u8)
                .dmadstincr()
                .bits(dst_incr as u8)
                .dmaen()
                .clear_bit()
        });
    }

    /// Configures trigger source
    pub fn set_trigger(&self, trigger: TriggerSource, internal_trigger: bool) {
        Self::check_channel();
        dma().regs.dma_dmatrig(N).dma_dmatctl().write(|w| unsafe {
            w.dmatsel()
                .bits(trigger as u8)
                .dmatint()
                .bit(internal_trigger)
        });
    }

    /// Enables DMA channel
    pub fn enable(&self) {
        Self::check_channel();
        dma().regs
            .dma_dmachan(N)
            .dma_dmactl()
            .modify(|_, w| w.dmaen().set_bit());
    }

    /// Disables DMA channel
    pub fn disable(&self) {
        Self::check_channel();
        dma().regs
            .dma_dmachan(N)
            .dma_dmactl()
            .modify(|_, w| w.dmaen().clear_bit());
    }

    /// Software-trigger transfer request
    pub fn software_trigger(&self) {
        Self::check_channel();
        dma().regs
            .dma_dmachan(N)
            .dma_dmactl()
            .modify(|_, w| w.dmareq().set_bit());
    }

    /// Returns true when transfer size has counted down to zero
    pub fn is_done(&self) -> bool {
        Self::check_channel();
        dma().regs.dma_dmachan(N).dma_dmasz().read().size().bits() == 0
    }

    /// Clears DMA done interrupt flag for this channel
    pub fn clear_done_flag(&self) {
        Self::check_channel();
        dma().regs
            .dma_cpu_int(0)
            .dma_cpu_int_iclr()
            .write(|w| unsafe { w.bits(1u32 << N) });
    }
}

/// Constructor for channel handle
pub const fn dma_channel<const N: usize>() -> DmaChannel<N> {
    DmaChannel::new()
}

macro_rules! dma_channel_aliases {
    ($($n:literal),+ $(,)?) => {
        $(
            pub type ${concat(DmaChannel, $n)} = DmaChannel<$n>;

            pub const fn ${concat(dma_channel, $n)}() -> ${concat(DmaChannel, $n)} {
                DmaChannel::new()
            }
        )+
    };
}

dma_channel_aliases!(0, 1, 2, 3, 4, 5, 6);