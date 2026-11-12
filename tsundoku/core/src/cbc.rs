//! AES-128 in CBC mode, decrypting only: MEGA's file and folder names (`hosts::mega`) and the links a Click'n'Load
//! button sends (`click_n_load`).

use aes::Aes128;
use aes::cipher::{BlockCipherDecrypt, KeyInit};

/// `encrypted` decrypted with `key`, starting from `iv`. Its length must be a multiple of 16; a partial last block is
/// left out.
pub(crate) fn decrypt(key: &[u8; 16], iv: [u8; 16], encrypted: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new(&(*key).into());
    let mut previous = iv;
    let mut plain = Vec::with_capacity(encrypted.len());
    for block in encrypted.as_chunks::<16>().0 {
        let mut decrypted = (*block).into();
        cipher.decrypt_block(&mut decrypted);
        let decrypted: [u8; 16] = decrypted.into();
        plain.extend(
            decrypted
                .iter()
                .zip(previous)
                .map(|(byte, previous)| byte ^ previous),
        );
        previous = *block;
    }
    plain
}
