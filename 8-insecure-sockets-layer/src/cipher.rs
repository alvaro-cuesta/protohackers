use std::num::Wrapping;

pub trait Cipher {
    fn cipher(&mut self, byte: u8) -> u8;

    fn decipher(&mut self, byte: u8) -> u8;
}

#[derive(Debug, Clone)]
pub struct ReverseBitsCipher;

impl Cipher for ReverseBitsCipher {
    fn cipher(&mut self, byte: u8) -> u8 {
        byte.reverse_bits()
    }

    fn decipher(&mut self, byte: u8) -> u8 {
        byte.reverse_bits()
    }
}

#[derive(Debug, Clone)]
pub struct XorCipher(u8);

impl Cipher for XorCipher {
    fn cipher(&mut self, byte: u8) -> u8 {
        byte ^ self.0
    }

    fn decipher(&mut self, byte: u8) -> u8 {
        byte ^ self.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct XorPosCipher {
    cipher_pos: Wrapping<u8>,
    decipher_pos: Wrapping<u8>,
}

impl Cipher for XorPosCipher {
    fn cipher(&mut self, byte: u8) -> u8 {
        let ciphered = byte ^ self.cipher_pos.0;
        self.cipher_pos += 1;
        ciphered
    }

    fn decipher(&mut self, byte: u8) -> u8 {
        let deciphered = byte ^ self.decipher_pos.0;
        self.decipher_pos += 1;
        deciphered
    }
}

#[derive(Debug, Clone)]
pub struct AddCipher(u8);

impl Cipher for AddCipher {
    fn cipher(&mut self, byte: u8) -> u8 {
        byte.wrapping_add(self.0)
    }

    fn decipher(&mut self, byte: u8) -> u8 {
        byte.wrapping_sub(self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AddPosCipher {
    cipher_pos: Wrapping<u8>,
    decipher_pos: Wrapping<u8>,
}

impl Cipher for AddPosCipher {
    fn cipher(&mut self, byte: u8) -> u8 {
        let ciphered = byte.wrapping_add(self.cipher_pos.0);
        self.cipher_pos += 1;
        ciphered
    }

    fn decipher(&mut self, byte: u8) -> u8 {
        let deciphered = byte.wrapping_sub(self.decipher_pos.0);
        self.decipher_pos += 1;
        deciphered
    }
}

#[derive(Debug, Clone)]
pub enum ComposableCipher {
    ReverseBits(ReverseBitsCipher),
    Xor(XorCipher),
    XorPos(XorPosCipher),
    Add(AddCipher),
    AddPos(AddPosCipher),
}

impl ComposableCipher {
    pub fn from_spec_slice(spec_slice: &[u8]) -> Option<(Self, &[u8])> {
        use ComposableCipher::*;

        let id = *spec_slice.get(0)?;

        match id {
            0x01 => Some((ReverseBits(ReverseBitsCipher), &spec_slice[1..])),
            0x02 => Some((Xor(XorCipher(*spec_slice.get(1)?)), &spec_slice[2..])),
            0x03 => Some((XorPos(XorPosCipher::default()), &spec_slice[1..])),
            0x04 => Some((Add(AddCipher(*spec_slice.get(1)?)), &spec_slice[2..])),
            0x05 => Some((AddPos(AddPosCipher::default()), &spec_slice[1..])),
            _ => None,
        }
    }
}

impl Cipher for ComposableCipher {
    fn cipher(&mut self, byte: u8) -> u8 {
        use ComposableCipher::*;

        match self {
            ReverseBits(cipher) => cipher.cipher(byte),
            Xor(cipher) => cipher.cipher(byte),
            XorPos(cipher) => cipher.cipher(byte),
            Add(cipher) => cipher.cipher(byte),
            AddPos(cipher) => cipher.cipher(byte),
        }
    }

    fn decipher(&mut self, byte: u8) -> u8 {
        use ComposableCipher::*;

        match self {
            ReverseBits(cipher) => cipher.decipher(byte),
            Xor(cipher) => cipher.decipher(byte),
            XorPos(cipher) => cipher.decipher(byte),
            Add(cipher) => cipher.decipher(byte),
            AddPos(cipher) => cipher.decipher(byte),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComposedCipher(Vec<ComposableCipher>);

impl ComposedCipher {
    pub fn from_spec_slice(mut spec_slice: &[u8]) -> Option<Self> {
        let mut ciphers = Vec::new();

        while !spec_slice.is_empty() {
            let (cipher, new_spec_slice) = ComposableCipher::from_spec_slice(spec_slice)?;
            ciphers.push(cipher);
            spec_slice = new_spec_slice;
        }

        Some(Self(ciphers))
    }

    pub fn check_is_noop(&self) -> bool {
        let mut cloned = self.clone();

        let test_buf = [0; 255].map(|byte| cloned.cipher(byte));

        test_buf == [0; 255]
    }
}

impl Cipher for ComposedCipher {
    fn cipher(&mut self, mut byte: u8) -> u8 {
        for cipher in self.0.iter_mut() {
            byte = cipher.cipher(byte);
        }

        byte
    }

    fn decipher(&mut self, mut byte: u8) -> u8 {
        for cipher in self.0.iter_mut().rev() {
            byte = cipher.decipher(byte);
        }

        byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex() {
        let mut composed_cipher =
            ComposedCipher::from_spec_slice(&[0x02, 73, 0x05, 0x04, 177]).unwrap();

        dbg!(&composed_cipher);

        let ciphered = composed_cipher.cipher(0);

        dbg!(ciphered);

        assert_eq!(composed_cipher.decipher(ciphered), 0);
    }
}
