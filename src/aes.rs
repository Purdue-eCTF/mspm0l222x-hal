use cortex_m::asm::nop;
use mspm0l222x_pac::Aesadv as PacAesadv;
use once_cell::sync::OnceCell;

use crate::{PWREN_WRITE_KEY, RSTCTL_WRITE_KEY};

static AES: OnceCell<AesAdv> = OnceCell::new();

pub fn aes() -> &'static AesAdv {
    AES.get().expect("AES not initialized")
}

pub struct AesAdv {
    aes: PacAesadv,
}

unsafe impl Send for AesAdv {}
unsafe impl Sync for AesAdv {}

impl AesAdv {
    fn new(aes: PacAesadv) -> Self {
        aes.aesadv_gprcm(0).aesadv_rstctl().write(|w| {
            unsafe { w.bits(RSTCTL_WRITE_KEY) }
                .resetassert()
                .assert()
                .resetstkyclr()
                .clr()
        });

        aes.aesadv_gprcm(0)
            .aesadv_pwren()
            .write(|w| unsafe { w.bits(PWREN_WRITE_KEY) }.enable().set_bit());

        AesAdv { aes }
    }

    pub fn init(aes: PacAesadv) {
        let _ = AES.get_or_init(|| AesAdv::new(aes));
    }

    pub fn set_key(&self, key: &[u8; 16]) {
        while self.aes.aesadv_ctrl().read().cntxt_rdy().is_notready() {}

        let key: &[u32; 4] = bytemuck::cast_ref(key);

        self.aes.aesadv_key0().write(|w| unsafe { w.bits(key[0]) });
        self.aes.aesadv_key1().write(|w| unsafe { w.bits(key[1]) });
        self.aes.aesadv_key2().write(|w| unsafe { w.bits(key[2]) });
        self.aes.aesadv_key3().write(|w| unsafe { w.bits(key[3]) });
    }

    pub fn encrypt_block(&self, plaintext: &[u8; 16]) -> [u8; 16] {
        self.process_block(plaintext, true)
    }

    fn process_block(&self, input_data: &[u8; 16], is_encrypt: bool) -> [u8; 16] {
        let mut output_data = [0u8; 16];
        self.aes
            .aesadv_ctrl()
            .write(|w| w.keysize().k128().dir().bit(is_encrypt));
        self.aes
            .aesadv_c_length_0()
            .write(|w| unsafe { w.bits(16) });

        while self.aes.aesadv_ctrl().read().cntxt_rdy().is_notready() {
            nop();
        }

        let input_data: &[u32; 4] = bytemuck::cast_ref(input_data);

        self.aes
            .aesadv_data0()
            .write(|w| unsafe { w.bits(input_data[0]) });
        self.aes
            .aesadv_data1()
            .write(|w| unsafe { w.bits(input_data[1]) });
        self.aes
            .aesadv_data2()
            .write(|w| unsafe { w.bits(input_data[2]) });
        self.aes
            .aesadv_data3()
            .write(|w| unsafe { w.bits(input_data[3]) });

        while self.aes.aesadv_ctrl().read().output_rdy().is_notready() {
            nop();
        }

        let output_data_view: &mut [u32; 4] = bytemuck::cast_mut(&mut output_data);
        output_data_view[0] = self.aes.aesadv_data0().read().bits();
        output_data_view[1] = self.aes.aesadv_data1().read().bits();
        output_data_view[2] = self.aes.aesadv_data2().read().bits();
        output_data_view[3] = self.aes.aesadv_data3().read().bits();

        output_data
    }
}
