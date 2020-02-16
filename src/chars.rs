use std::char;
use std::io;
use std::iter::Iterator;

pub struct CharIter<B>(pub B);

impl<B: io::BufRead> CharIter<B> {
    fn read_one(&mut self) -> Option<u8> {
        let res = self.0.fill_buf();
        if let Err(err) = res {
            error!("input io: {}", err);
            std::process::exit(1);
        }
        let buf = res.unwrap();
        if buf.is_empty() {
            return None;
        }
        let ret = buf[0];
        self.0.consume(1);
        Some(ret)
    }
}

impl<B: io::BufRead> Iterator for CharIter<B> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        let n1 = self.read_one()?;
        let i = u32::from_be_bytes([0x00, 0x00, 0x00, n1]);
        if let Some(c) = char::from_u32(i) {
            return Some(c);
        }
        let n2 = self.read_one()?;
        let i = u32::from_be_bytes([0x00, 0x00, n2, n1]);
        if let Some(c) = char::from_u32(i) {
            return Some(c);
        }
        let n3 = self.read_one()?;
        let i = u32::from_be_bytes([0x00, n3, n2, n1]);
        if let Some(c) = char::from_u32(i) {
            return Some(c);
        }
        let n4 = self.read_one()?;
        let i = u32::from_be_bytes([n4, n3, n2, n1]);
        if let Some(c) = char::from_u32(i) {
            return Some(c);
        }
        error!("Failed to read utf-8 char");
        std::process::exit(1);
    }
}
