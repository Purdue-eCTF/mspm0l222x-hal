use once_cell::sync::OnceCell;
use cortex_m::asm::nop;

/// AES base address (Datasheet Table 8-5)
pub const AES_BASE_ADDR: usize = 0x40442000;

// offsets
const REG_PWREN:        isize = 0x800;
const REG_RSTCTL:       isize = 0x804;
const REG_AES_KEY0:     isize = 0x1120; 
const REG_AES_CTRL:     isize = 0x1150; 
const REG_AES_C_LEN0:   isize = 0x1154; 
const REG_AES_DATA0:    isize = 0x1160; 

const PWREN_KEY: u32 = 0x26;
const RSTCTL_KEY: u32 = 0xB1;

// CTRL bitmasks
const CTRL_CNTXT_RDY:  u32 = 1 << 31;
const CTRL_INPUT_RDY:  u32 = 1 << 1;
const CTRL_OUTPUT_RDY: u32 = 1 << 0;
const CTRL_DIR_ENCRYPT: u32 = 1 << 2;
const CTRL_KEYSIZE_128: u32 = 1 << 3; 

static AES: OnceCell<AesAdv> = OnceCell::new();

/// Initializes aes hardware, creating and returning an instance of Uart; panics if initialization fails.
pub fn aes() -> &'static AesAdv {
    AES.get().expect("AES not initialized")
}

/// The AESADV accelerates encryption and decryption in hardware.
pub struct AesAdv {
    base_addr: usize,
}

unsafe impl Send for AesAdv {}
unsafe impl Sync for AesAdv {}

impl AesAdv {
    // Creates a new AesAdv instance.
    fn new(base_addr: usize) -> Self {
        AesAdv { base_addr }
    }

    /// Initiate the hardware trng unit if it isn't already initiated before retrieving the trng unit.
    pub fn init() {
        let _ = AES.get_or_init(|| {
            let instance = AesAdv::new(AES_BASE_ADDR);
            instance.hw_init();
            instance
        });
    }

    // write to aes
    unsafe fn write_reg(&self, offset: isize, value: u32) {
        let addr = (self.base_addr as *mut u32).offset(offset / 4);
        addr.write_volatile(value);
    }

    // read from aes (and return values that were read)
    unsafe fn read_reg(&self, offset: isize) -> u32 {
        let addr = (self.base_addr as *mut u32).offset(offset / 4);
        addr.read_volatile()
    }

    // TODO
    fn hw_init(&self) {
        unsafe {
            // power and reset sequence
            self.write_reg(REG_RSTCTL, (RSTCTL_KEY << 24) | 1); // clear reset sticky bit
            for _ in 0..100 { nop(); }
            self.write_reg(REG_RSTCTL, (RSTCTL_KEY << 24) | 0); // assert reset
            for _ in 0..1000 { nop(); }
            self.write_reg(REG_PWREN, (PWREN_KEY << 24) | 1); // enable power to hardware
            for _ in 0..5000 { nop(); }
        }
    }

    /// Load key into AES and return true on completion.
    pub fn set_key(&self, key: &[u32; 4]) -> bool {
        unsafe {
            // while aes and context data registers are in use, do not continue
            while (self.read_reg(REG_AES_CTRL) & CTRL_CNTXT_RDY) == 0 { nop(); }
            
            // load key
            for (i, &k) in key.iter().enumerate() {
                self.write_reg(REG_AES_KEY0 + (i as isize * 4), k.swap_bytes());
            }
        }
        true
    }

    /// Encrypt a block (array) of data and return the encrypted data.
    pub fn encrypt_block(&self, plaintext: &[u32; 4]) -> [u32; 4] {
        self.process_block(plaintext, true)
    }

    // encrypt a block (array) of data and return the encrypted data
    fn process_block(&self, input_data: &[u32; 4], is_encrypt: bool) -> [u32; 4] {
        let mut output_data = [0u32; 4];
        
        unsafe {
            let mut ctrl = CTRL_KEYSIZE_128;
            if is_encrypt {
                ctrl |= CTRL_DIR_ENCRYPT;
            }
            self.write_reg(REG_AES_CTRL, ctrl);

            self.write_reg(REG_AES_C_LEN0, 16); 

            while (self.read_reg(REG_AES_CTRL) & CTRL_INPUT_RDY) == 0 { nop(); }

            for (i, &val) in input_data.iter().enumerate() {
                self.write_reg(REG_AES_DATA0 + (i as isize * 4), val.swap_bytes());
            }

            while (self.read_reg(REG_AES_CTRL) & CTRL_OUTPUT_RDY) == 0 { nop(); }

            for (i, val) in output_data.iter_mut().enumerate() {
                *val = self.read_reg(REG_AES_DATA0 + (i as isize * 4)).swap_bytes();
            }
        }

        output_data
    }
}
