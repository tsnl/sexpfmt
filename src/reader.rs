// TODO: rewrite this module: expose a 'CharReader' that accumulates bytes, returns utf-32 chars.
// This will allow for more robust whitespace recognition, skipping, and column counts (grapheme clusters, technically?).

use super::*;
use std::io;

pub struct FormReader<R: io::Read> {
	inner: ByteReader<R>,
}

impl<R: io::Read> FormReader<R> {
	pub fn new(inner: R) -> io::Result<Self> {
		Ok(Self {
			inner: ByteReader::new(inner)?,
		})
	}
}
impl<R: io::Read> FormReader<R> {
	fn is_whitespace_byte(b: u8) -> bool {
		match b {
			b' ' | b'\n' | b'\r' | b'\t' | 0x0B => true, // '\v'
			_ => false,
		}
	}
}
impl<R: io::Read> FormReader<R> {
	pub fn get(&mut self) -> Result<Option<(String, Loc)>> {
		self.skip_whitespace_prefix()?;
		let position = self.inner.peek_loc();
		self.get_without_whitespace_prefix(position)
	}
	fn skip_whitespace_prefix(&mut self) -> Result<()> {
		loop {
			match self.inner.peek() {
				Some(b) if Self::is_whitespace_byte(b) => {
					_ = self.inner.get()?;
					continue;
				}
				_ => {
					return Ok(());
				}
			}
		}
	}
	fn get_without_whitespace_prefix(&mut self, position: Loc) -> Result<Option<(String, Loc)>> {
		match self.inner.peek() {
			Some(b'(') | Some(b')') => self.get_list_without_whitespace_prefix(position),
			Some(b'[') | Some(b']') => self.get_list_without_whitespace_prefix(position),
			Some(b'{') | Some(b'}') => self.get_list_without_whitespace_prefix(position),
			Some(_) => self.get_atom_without_whitespace_prefix(position),
			None => Ok(None),
		}
	}
	fn get_list_without_whitespace_prefix(&mut self, position: Loc) -> Result<Option<(String, Loc)>> {
		let mut bookend_stack = Vec::default();
		let mut bytes = Vec::default();

		macro_rules! pop_bookend {
			($x:expr) => {
				match bookend_stack.pop() {
					Some(it) => {
						// Expect TOS
						if it != $x {
							return Err(SexpfmtError::mismatched_bookends(position.clone(), it, $x));
						}
						// If bookend stack is empty after popping, conclude this form.
						if bookend_stack.is_empty() {
							let string = String::from_utf8(bytes).map_err(|e| {
								SexpfmtError::form_reader_error(
									"Invalid UTF-8 in list",
									Some(position.clone()),
									Some(Box::new(e)),
								)
							})?;
							return Ok(Some((string, position)));
						}
					}
					None => {
						return Err(SexpfmtError::invalid_input(
							format!("Unexpected closing bookend {:#?}", $x),
							position.clone(),
						));
					}
				}
			};
		}

		macro_rules! handle_eof {
			() => {
				if !bookend_stack.is_empty() {
					return Err(SexpfmtError::unexpected_eof(
						position.clone(),
						bookend_stack.len(),
					));
				} else {
					return Ok(None);
				}
			};
		}

		loop {
			let b = self.inner.get()?;

			if let Some(b) = b {
				bytes.push(b);
			}

			match b {
				Some(b'(') => bookend_stack.push(SExpBookendStyle::Parentheses),
				Some(b'[') => bookend_stack.push(SExpBookendStyle::SquareBrackets),
				Some(b'{') => bookend_stack.push(SExpBookendStyle::CurlyBraces),
				Some(b')') => pop_bookend!(SExpBookendStyle::Parentheses),
				Some(b']') => pop_bookend!(SExpBookendStyle::SquareBrackets),
				Some(b'}') => pop_bookend!(SExpBookendStyle::CurlyBraces),
				None => handle_eof!(),
				Some(_) => {}
			}
		}
	}
	fn get_atom_without_whitespace_prefix(&mut self, position: Loc) -> Result<Option<(String, Loc)>> {
		let mut bytes = Vec::default();
		loop {
			match self.inner.get()? {
				Some(b) if !Self::is_whitespace_byte(b) => {
					bytes.push(b);
				}
				_ => {
					let string = String::from_utf8(bytes).map_err(|e| {
						SexpfmtError::form_reader_error(
							"Invalid UTF-8 in atom",
							Some(position),
							Some(Box::new(e)),
						)
					})?;
					return Ok(Some((string, position)));
				}
			}
		}
	}
}

struct ByteReader<R: io::Read> {
	inner: R,
	peek: Option<u8>,
	peek_loc: Loc,
}

impl<R: io::Read> ByteReader<R> {
	fn new(inner: R) -> io::Result<Self> {
		let mut v = Self {
			inner,
			peek: None,
			peek_loc: Loc::new(0, 1, 1),
		};
		assert_eq!(None, v.get()?);
		Ok(v)
	}
}
impl<R: io::Read> ByteReader<R> {
	fn get(&mut self) -> io::Result<Option<u8>> {
		let b = self.get_without_peek()?;
		let v = self.peek;
		self.peek = b;

		// Update position tracking for the byte we're returning
		if let Some(byte) = v {
			self.peek_loc = Self::next_loc(self.peek_loc, byte);
		}

		Ok(v)
	}
	fn get_without_peek(&mut self) -> io::Result<Option<u8>> {
		let mut buf: [u8; 1] = Default::default();
		let n = self.inner.read(&mut buf)?;
		if n > 0 {
			Ok(Some(buf[0]))
		} else {
			// EOF
			Ok(None)
		}
	}
	fn peek(&self) -> Option<u8> {
		self.peek
	}
	fn peek_loc(&self) -> Loc {
		self.peek_loc
	}

	fn next_loc(old: Loc, byte: u8) -> Loc {
		let new_offset = old.offset() + 1;
		if byte == b'\n' {
			Loc::new(new_offset, old.line() + 1, 1)
		} else {
			Loc::new(new_offset, old.line(), old.column() + 1)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use stringreader::StringReader;

	macro_rules! case {
		($s:expr, $v:expr) => {
			let r = StringReader::new($s);
			let mut r = FormReader::new(r).unwrap();
			for x in ($v).into_iter() {
				let result = r.get().unwrap();
				assert!(result.is_some());
				assert_eq!(result.unwrap().0, String::from(x));
			}
			assert_eq!(r.get().unwrap(), None);
		};
	}

	#[test]
	fn test_form_reader_1() {
		case!("()", vec!["()"]);
	}

	#[test]
	fn test_form_reader_2() {
		case!("[]", vec!["[]"]);
	}

	#[test]
	fn test_form_reader_3() {
		case!("{}", vec!["{}"]);
	}

	#[test]
	fn test_form_reader_4() {
		case!("(a b c ())", vec!["(a b c ())"]);
	}

	#[test]
	fn test_form_reader_5() {
		case!("(a b c ()) (1 2 3)", vec!["(a b c ())", "(1 2 3)"]);
	}

	#[test]
	fn test_form_reader_6() {
		case!("1 2 3", vec!["1", "2", "3"]);
	}

	#[test]
	fn test_form_reader_position_tracking() {
		let input = "hello\n(world\n  foo)\nbar";
		let r = StringReader::new(input);
		let mut r = FormReader::new(r).unwrap();

		// First form: "hello" at line 1, column 1
		let result = r.get().unwrap().unwrap();
		assert_eq!(result.0, "hello");
		assert_eq!(result.1.line(), 1);
		assert_eq!(result.1.column(), 1);
		assert_eq!(result.1.offset(), 0);

		// Second form: "(world\n  foo)" at line 2, column 1
		let result = r.get().unwrap().unwrap();
		assert_eq!(result.0, "(world\n  foo)");
		assert_eq!(result.1.line(), 2);
		assert_eq!(result.1.column(), 1);
		assert_eq!(result.1.offset(), 6);

		// Third form: "bar" at line 4, column 1
		let result = r.get().unwrap().unwrap();
		assert_eq!(result.0, "bar");
		assert_eq!(result.1.line(), 4);
		assert_eq!(result.1.column(), 1);
		assert_eq!(result.1.offset(), 20);

		// EOF
		assert_eq!(r.get().unwrap(), None);
	}

	#[test]
	fn test_form_reader_atom_ending_position() {
		// Test atom ending with whitespace vs EOF
		let input = "atom1 atom2";
		let r = StringReader::new(input);
		let mut r = FormReader::new(r).unwrap();

		// First atom: "atom1" at line 1, column 1
		let result = r.get().unwrap().unwrap();
		assert_eq!(result.0, "atom1");
		assert_eq!(result.1.line(), 1);
		assert_eq!(result.1.column(), 1);
		assert_eq!(result.1.offset(), 0);

		// Second atom: "atom2" at line 1, column 7 (after "atom1 ")
		let result = r.get().unwrap().unwrap();
		assert_eq!(result.0, "atom2");
		assert_eq!(result.1.line(), 1);
		assert_eq!(result.1.column(), 7);
		assert_eq!(result.1.offset(), 6);

		// EOF
		assert_eq!(r.get().unwrap(), None);
	}
}
