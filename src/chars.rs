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
        if n1 & 0b1111_1000 == 0b1111_0000 {
            let n1 = n1 & 0b0000_0111;
            let n2 = self.read_one()? & 0b0011_1111;
            let n3 = self.read_one()? & 0b0011_1111;
            let n4 = self.read_one()? & 0b0011_1111;
            let n = ((n1 as u32) << 18) | ((n2 as u32) << 12) | ((n3 as u32) << 6) | (n4 as u32);
            if let Some(c) = char::from_u32(n) {
                return Some(c);
            }
        }
        if n1 & 0b1111_0000 == 0b1110_0000 {
            let n1 = n1 & 0b0000_1111;
            let n2 = self.read_one()? & 0b0011_1111;
            let n3 = self.read_one()? & 0b0011_1111;
            let n = ((n1 as u32) << 12) | ((n2 as u32) << 6) | (n3 as u32);
            if let Some(c) = char::from_u32(n) {
                return Some(c);
            }
        }
        if n1 & 0b1110_0000 == 0b1100_0000 {
            let n1 = n1 & 0b0001_1111;
            let n2 = self.read_one()? & 0b0011_1111;
            let n = ((n1 as u32) << 6) | (n2 as u32);
            if let Some(c) = char::from_u32(n) {
                return Some(c);
            }
        }
        if n1 & 0b1000_0000 == 0b0000_0000 {
            let n = (n1 & 0b0111_1111) as u32;
            if let Some(c) = char::from_u32(n) {
                return Some(c);
            }
        }
        error!("Failed to read utf-8 char");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// 3 bytes per char
    #[test]
    fn test_japanese() {
        let s = "おはよう世界";
        let curs = io::Cursor::new(s.as_bytes());
        let reader = io::BufReader::new(curs);
        let mut iter = CharIter(reader);
        assert_eq!(iter.next(), Some('お'));
        assert_eq!(iter.next(), Some('は'));
        assert_eq!(iter.next(), Some('よ'));
        assert_eq!(iter.next(), Some('う'));
        assert_eq!(iter.next(), Some('世'));
        assert_eq!(iter.next(), Some('界'));
        assert_eq!(iter.next(), None);
    }

    /// 4 bytes per char
    #[test]
    fn test_emoji() {
        let s = "💚🙈🌈";
        let curs = io::Cursor::new(s.as_bytes());
        let reader = io::BufReader::new(curs);
        let mut iter = CharIter(reader);
        assert_eq!(iter.next(), Some('💚'));
        assert_eq!(iter.next(), Some('🙈'));
        assert_eq!(iter.next(), Some('🌈'));
        assert_eq!(iter.next(), None);
    }

    /// 1 byte per char
    #[test]
    fn test_ascii() {
        let s = "abc";
        let curs = io::Cursor::new(s.as_bytes());
        let reader = io::BufReader::new(curs);
        let mut iter = CharIter(reader);
        assert_eq!(iter.next(), Some('a'));
        assert_eq!(iter.next(), Some('b'));
        assert_eq!(iter.next(), Some('c'));
        assert_eq!(iter.next(), None);
    }

    /// 2 bytes per char
    #[test]
    fn test_greek() {        
        let s = "ΔΣψ";
        let curs = io::Cursor::new(s.as_bytes());
        let reader = io::BufReader::new(curs);
        let mut iter = CharIter(reader);
        assert_eq!(iter.next(), Some('Δ'));
        assert_eq!(iter.next(), Some('Σ'));
        assert_eq!(iter.next(), Some('ψ'));
        assert_eq!(iter.next(), None);
    }
}
