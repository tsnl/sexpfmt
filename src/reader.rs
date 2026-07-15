use crate::{Loc, Result, SexpfmtError};
use std::io;
use std::io::Read;

/// An incremental UTF-8 decoder over any [`io::Read`], with one-character
/// lookahead and location (byte offset, line, column) tracking.
///
/// The inner reader is buffered internally, so callers do not need to wrap it
/// in an [`io::BufReader`].
pub(crate) struct CharReader<R: io::Read> {
	inner: io::BufReader<R>,
	peeked: Option<char>,
	/// Location of the next character to be consumed (the peeked one, when
	/// `peeked` is `Some`), or of EOF once the input is exhausted.
	loc: Loc,
}

impl<R: io::Read> CharReader<R> {
	pub(crate) fn new(inner: R) -> Self {
		Self {
			inner: io::BufReader::new(inner),
			peeked: None,
			loc: Loc::new(0, 1, 1),
		}
	}

	/// Location of the character [`Self::peek`] would return, i.e. of the next
	/// character to be consumed.
	pub(crate) fn loc(&self) -> Loc {
		self.loc
	}

	pub(crate) fn peek(&mut self) -> Result<Option<char>> {
		if self.peeked.is_none() {
			self.peeked = self.decode_char()?;
		}
		Ok(self.peeked)
	}

	pub(crate) fn next(&mut self) -> Result<Option<char>> {
		let c = self.peek()?;
		self.peeked = None;
		if let Some(c) = c {
			self.loc = self.loc.advanced_by(c);
		}
		Ok(c)
	}

	fn read_byte(&mut self) -> Result<Option<u8>> {
		let mut buf = [0u8; 1];
		loop {
			match self.inner.read(&mut buf) {
				Ok(0) => return Ok(None),
				Ok(_) => return Ok(Some(buf[0])),
				Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
				Err(e) => return Err(e.into()),
			}
		}
	}

	fn decode_char(&mut self) -> Result<Option<char>> {
		let start = self.loc;
		let Some(b0) = self.read_byte()? else {
			return Ok(None);
		};
		let len = match b0 {
			0x00..=0x7F => return Ok(Some(b0 as char)),
			0xC0..=0xDF => 2,
			0xE0..=0xEF => 3,
			0xF0..=0xF7 => 4,
			// Continuation bytes and bytes never valid in UTF-8.
			_ => return Err(SexpfmtError::invalid_utf8(start)),
		};
		let mut buf = [b0, 0, 0, 0];
		for slot in buf.iter_mut().take(len).skip(1) {
			*slot = self
				.read_byte()?
				.ok_or_else(|| SexpfmtError::invalid_utf8(start))?;
		}
		// `from_utf8` rejects malformed continuation bytes, overlong encodings,
		// surrogates, and out-of-range scalar values.
		match std::str::from_utf8(&buf[..len]) {
			Ok(s) => Ok(s.chars().next()),
			Err(_) => Err(SexpfmtError::invalid_utf8(start)),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_ascii_and_multibyte_decoding() {
		let mut r = CharReader::new("aé日🦀".as_bytes());
		assert_eq!(r.next().unwrap(), Some('a'));
		assert_eq!(r.next().unwrap(), Some('é'));
		assert_eq!(r.next().unwrap(), Some('日'));
		assert_eq!(r.next().unwrap(), Some('🦀'));
		assert_eq!(r.next().unwrap(), None);
		assert_eq!(r.next().unwrap(), None);
	}

	#[test]
	fn test_peek_does_not_consume() {
		let mut r = CharReader::new("ab".as_bytes());
		assert_eq!(r.peek().unwrap(), Some('a'));
		assert_eq!(r.peek().unwrap(), Some('a'));
		assert_eq!(r.next().unwrap(), Some('a'));
		assert_eq!(r.peek().unwrap(), Some('b'));
	}

	#[test]
	fn test_loc_tracking() {
		let mut r = CharReader::new("aé\nb".as_bytes());
		assert_eq!(r.loc(), Loc::new(0, 1, 1));
		r.next().unwrap(); // 'a'
		assert_eq!(r.loc(), Loc::new(1, 1, 2));
		r.next().unwrap(); // 'é': 2 bytes, 1 column
		assert_eq!(r.loc(), Loc::new(3, 1, 3));
		r.next().unwrap(); // '\n'
		assert_eq!(r.loc(), Loc::new(4, 2, 1));
		r.next().unwrap(); // 'b'
		assert_eq!(r.loc(), Loc::new(5, 2, 2));
	}

	#[test]
	fn test_peek_does_not_advance_loc() {
		let mut r = CharReader::new("ab".as_bytes());
		r.next().unwrap();
		let loc = r.loc();
		r.peek().unwrap();
		assert_eq!(r.loc(), loc);
	}

	#[test]
	fn test_invalid_utf8_byte() {
		let mut r = CharReader::new(&b"a\xffb"[..]);
		assert_eq!(r.next().unwrap(), Some('a'));
		match r.next() {
			Err(SexpfmtError::InvalidUtf8 { position }) => {
				assert_eq!(position, Loc::new(1, 1, 2));
			}
			other => panic!("expected InvalidUtf8, got {other:?}"),
		}
	}

	#[test]
	fn test_truncated_utf8_sequence() {
		// The first two bytes of '€' (0xE2 0x82 0xAC), truncated at EOF.
		let mut r = CharReader::new(&b"\xe2\x82"[..]);
		assert!(matches!(
			r.next(),
			Err(SexpfmtError::InvalidUtf8 { position }) if position == Loc::new(0, 1, 1)
		));
	}

	#[test]
	fn test_lone_continuation_byte() {
		let mut r = CharReader::new(&b"\x80"[..]);
		assert!(matches!(r.next(), Err(SexpfmtError::InvalidUtf8 { .. })));
	}

	#[test]
	fn test_overlong_encoding_rejected() {
		// 0xC0 0x80 is an overlong encoding of NUL.
		let mut r = CharReader::new(&b"\xc0\x80"[..]);
		assert!(matches!(r.next(), Err(SexpfmtError::InvalidUtf8 { .. })));
	}
}
