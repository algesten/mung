use crate::chars::CharIter;
use std::fmt;
use std::io;
use std::iter::Iterator;
use std::iter::Peekable;
use std::vec::IntoIter;

#[derive(Clone)]
pub struct Token {
    kind: TokenKind,
    s: String,
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T[{:?} {}]", self.kind, self.s)
    }
}

impl Token {
    pub fn is_whitespace(&self) -> bool {
        self.kind == TokenKind::Whitespace
    }

    pub fn is_name(&self) -> bool {
        self.s.chars().all(|c| char::is_ascii_alphabetic(&c))
    }

    pub fn expect_name(self) -> Result<String, String> {
        if self.is_name() {
            Ok(self.s)
        } else {
            Err(format!("Expected name: {}", self.s))
        }
    }

    pub fn expect_kind(self, kind: TokenKind) -> Result<Self, String> {
        if self.kind == kind {
            Ok(self)
        } else {
            Err(format!("Expected {:?} but got: {:?}", kind, self.kind))
        }
    }
}

pub struct Tokenizer<B: io::BufRead>(Peekable<CharIter<B>>, Option<Token>);

impl<B: io::BufRead> Tokenizer<B> {
    pub fn peek(&mut self) -> Option<&Token> {
        if self.1.is_none() {
            self.1 = self.next();
        }
        self.1.as_ref()
    }
}

impl<B: io::BufRead> Iterator for Tokenizer<B> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        // use up peeked token
        if self.1.is_some() {
            return self.1.take();
        }

        // if we are building a segment
        let mut cur_seg: Option<Token> = None;

        while let Some(c) = self.0.peek() {
            let kind = TokenKind::of(*c);

            if kind.is_segment() {
                if let Some(cur_seg_ref) = &mut cur_seg {
                    // we have a segment
                    if kind != cur_seg_ref.kind {
                        // other kind of segment.
                        return cur_seg.take();
                    } else {
                        // extend current segment
                        cur_seg_ref.s.push(*c);
                        self.0.next();
                    }
                } else {
                    // start new segment
                    let mut s = String::new();
                    s.push(*c);
                    self.0.next();
                    cur_seg = Some(Token { kind, s });
                }
            } else if let Some(cur_seg) = cur_seg {
                // end of segment
                return Some(cur_seg);
            } else {
                let mut s = String::new();
                s.push(*c);
                self.0.next();
                return Some(Token { kind, s });
            }
        }
        // end of input segment
        cur_seg.take()
    }
}

pub enum Tokens<B: io::BufRead> {
    Tokenizer(Tokenizer<B>),
    Peekable(Peekable<IntoIter<Token>>),
}

impl<B: io::BufRead> Tokens<B> {
    pub fn peek(&mut self) -> Option<&Token> {
        match self {
            Tokens::Tokenizer(t) => t.peek(),
            Tokens::Peekable(t) => t.peek(),
        }
    }
}

impl<B: io::BufRead> Iterator for Tokens<B> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Tokens::Tokenizer(t) => t.next(),
            Tokens::Peekable(t) => t.next(),
        }
    }
}

impl<B: io::BufRead> Tokens<B> {
    fn into_vec(self) -> Vec<Token> {
        self.collect()
    }

    pub fn into_string(self) -> String {
        let mut s = String::new();
        for t in self {
            s.push_str(&t.s[..]);
        }
        s
    }

    pub fn peek_kind(&mut self) -> Option<TokenKind> {
        self.peek().map(|t| t.kind)
    }

    pub fn skip_white(&mut self) {
        if let Some(x) = self.peek() {
            if x.is_whitespace() {
                self.next();
            }
        }
    }

    fn expect_something(&mut self) -> Result<Token, String> {
        if let Some(x) = self.next() {
            Ok(x)
        } else {
            Err("End of input".into())
        }
    }

    pub fn expect_name(&mut self) -> Result<String, String> {
        self.expect_something()?.expect_name()
    }

    pub fn expect_kind(&mut self, kind: TokenKind) -> Result<Token, String> {
        self.expect_something()?.expect_kind(kind)
    }

    pub fn expect_string(&mut self, keep: bool) -> Result<String, String> {
        let open = self.peek_kind().ok_or("End when we want a string")?;
        if !open.is_string_start() {
            return Err(format!("Expected string literal: {:?}", open));
        }
        Ok(self.find_pair(open, open, keep, true)?.into_string())
    }

    pub fn expect_as<F>(&mut self) -> Result<F, String>
    where
        F: std::str::FromStr,
        F::Err: std::error::Error,
    {
        self.expect_something()?
            .s
            .parse()
            .map_err(|e: F::Err| e.to_string())
    }

    pub fn find_pair(
        &mut self,
        start: TokenKind,
        end: TokenKind,
        keep: bool,
        use_string_escape: bool,
    ) -> Result<Tokens<B>, String> {
        let mut into = vec![];
        let stok = self.expect_kind(start)?;
        if keep {
            into.push(stok);
        }
        let mut level = 1;
        loop {
            // we might want to consume a string
            if let Some(peek) = self.peek() {
                if peek.kind.is_string_start() && peek.kind != start {
                    let kind = peek.kind;
                    let mut x = self.find_pair(kind, kind, true, true)?.into_vec();
                    into.append(&mut x);
                    continue;
                }
            }
            let cur = self.next();
            if cur.is_none() {
                break;
            }
            let cur = cur.unwrap();

            if use_string_escape && cur.kind == TokenKind::Backslash {
                // ignore next
                let next = self.next();
                if next.is_none() {
                    return Err("Pos {} unexpected end after string escape".into());
                }
                into.push(cur);
                into.push(next.unwrap());
                continue;
            }

            if cur.kind == end {
                level -= 1;
            } else if cur.kind == start {
                level += 1;
            }

            if level == 0 {
                if keep {
                    into.push(cur);
                }
                break;
            }

            into.push(cur);
        }
        if level > 0 {
            return Err(format!("Unbalanced {:?}-{:?}", start, end));
        }
        trace!("find_pair: {:?} {:?} {:?}", start, into, end);
        Ok(Tokens::Peekable(into.into_iter().peekable()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    CurlLeft,
    CurlRight,
    BracketLeft,
    BracketRight,
    ParenLeft,
    ParenRight,
    SingleQuote,
    DoubleQuote,
    Backslash,
    Comma,
    FullStop,
    Whitespace,
    Other,
}

impl TokenKind {
    fn of(c: char) -> TokenKind {
        match c {
            '{' => TokenKind::CurlLeft,
            '}' => TokenKind::CurlRight,
            '[' => TokenKind::BracketLeft,
            ']' => TokenKind::BracketRight,
            '(' => TokenKind::ParenLeft,
            ')' => TokenKind::ParenRight,
            '\'' => TokenKind::SingleQuote,
            '"' => TokenKind::DoubleQuote,
            '\\' => TokenKind::Backslash,
            ',' => TokenKind::Comma,
            '.' => TokenKind::FullStop,
            _ => {
                if c.is_whitespace() {
                    TokenKind::Whitespace
                } else {
                    TokenKind::Other
                }
            }
        }
    }

    fn is_segment(self) -> bool {
        match self {
            TokenKind::Whitespace | TokenKind::Other => true,
            _ => false,
        }
    }

    fn is_string_start(self) -> bool {
        match self {
            TokenKind::SingleQuote | TokenKind::DoubleQuote => true,
            _ => false,
        }
    }
}

pub fn tokenize<B: io::BufRead>(read: B) -> Tokens<B> {
    Tokens::Tokenizer(Tokenizer(CharIter(read).peekable(), None))
}

pub fn tokenize_str(s: &str) -> Tokens<io::BufReader<io::Cursor<&[u8]>>> {
    let cursor = io::Cursor::new(s.as_bytes());
    let reader = io::BufReader::new(cursor);
    tokenize(reader)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn tokenize_simple() {
        let tok = tokenize_str("db.teams.find({})");
        assert_eq!(
            format!("{:?}", tok.into_vec()),
            "[T[Other db], T[FullStop .], T[Other teams], \
                T[FullStop .], T[Other find], T[ParenLeft (], \
                T[CurlLeft {], T[CurlRight }], T[ParenRight )]]"
        );
    }

    #[test]
    fn tokenize_paren() {
        let tok = tokenize_str("skip(3)");
        assert_eq!(
            format!("{:?}", tok.into_vec()),
            "[T[Other skip], T[ParenLeft (], T[Other 3], T[ParenRight )]]"
        );
    }

    #[test]
    fn expect_double_string() {
        let mut tok = tokenize_str("\"foo\"");
        assert_eq!("foo", tok.expect_string(false).unwrap());
    }

    #[test]
    fn expect_single_string() {
        let mut tok = tokenize_str("'foo'");
        assert_eq!("foo", tok.expect_string(false).unwrap());
    }

    #[test]
    fn string_in_pair() {
        let mut tok = tokenize_str("{'foo'}");
        let x = tok
            .find_pair(TokenKind::CurlLeft, TokenKind::CurlRight, true, false)
            .unwrap();
        assert_eq!(
            format!("{:?}", x.into_vec()),
            "[T[CurlLeft {], T[SingleQuote '], T[Other foo], T[SingleQuote '], T[CurlRight }]]"
        );
    }

    #[test]
    fn string_in_pair_with_end() {
        let mut tok = tokenize_str("{' } '}");
        let x = tok
            .find_pair(TokenKind::CurlLeft, TokenKind::CurlRight, true, false)
            .unwrap();
        assert_eq!(
            format!("{:?}", x.into_vec()),
            "[T[CurlLeft {], T[SingleQuote '], T[Whitespace  ], T[CurlRight }], \
            T[Whitespace  ], T[SingleQuote '], T[CurlRight }]]"
        );
    }

    #[test]
    fn string_with_escape() {
        let mut tok = tokenize_str("' \\' '");
        assert_eq!(" \\' ", tok.expect_string(false).unwrap());
    }
}
