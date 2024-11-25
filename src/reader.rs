// TODO: rewrite this module: expose a 'CharReader' that accumulates bytes, returns utf-32 chars.
// This will allow for more robust whitespace recognition and skipping.

use std::error::Error;
use std::fmt::Display;
use std::io;
use std::result::Result;

use crate::SExpBookendStyle;

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
	pub fn get(&mut self) -> Result<Option<String>, FormReaderError> {
		self.skip_whitespace_prefix()?;
		self.get_without_whitespace_prefix()
	}
	fn skip_whitespace_prefix(&mut self) -> Result<(), FormReaderError> {
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
	fn get_without_whitespace_prefix(&mut self) -> Result<Option<String>, FormReaderError> {
		match self.inner.peek() {
			Some(b'(') | Some(b')') => self.get_list_without_whitespace_prefix(),
			Some(b'[') | Some(b']') => self.get_list_without_whitespace_prefix(),
			Some(b'{') | Some(b'}') => self.get_list_without_whitespace_prefix(),
			Some(_) => self.get_atom_without_whitespace_prefix(),
			None => Ok(None),
		}
	}
	fn get_list_without_whitespace_prefix(&mut self) -> Result<Option<String>, FormReaderError> {
		let mut bookend_stack = Vec::default();
		let mut bytes = Vec::default();

		macro_rules! pop_bookend {
			($x:expr) => {
				match bookend_stack.pop() {
					Some(it) => {
						// Expect TOS
						if it != $x {
							return Err(FormReaderError {
								message: format!("Mismatched bookends: got {:#?}, expected {:#?}", it, $x),
								cause: None,
							});
						}
						// If bookend stack is empty after popping, conclude this form.
						if bookend_stack.is_empty() {
							return Ok(Some(String::from_utf8(bytes)?));
						}
					}
					None => {
						return Err(FormReaderError {
							message: format!("Mismatched bookends: got unexpected opening {:#?}", $x),
							cause: None,
						})
					}
				}
			};
		}

		macro_rules! handle_eof {
			() => {
				if !bookend_stack.is_empty() {
					return Err(FormReaderError {
						message: format!("Unexpected EOF: {} un-closed bookends", bookend_stack.len()),
						cause: None,
					});
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
	fn get_atom_without_whitespace_prefix(&mut self) -> Result<Option<String>, FormReaderError> {
		let mut bytes = Vec::default();
		loop {
			match self.inner.get()? {
				Some(b) if !Self::is_whitespace_byte(b) => {
					bytes.push(b);
				}
				_ => {
					return Ok(Some(String::from_utf8(bytes)?));
				}
			}
		}
	}
}

struct ByteReader<R: io::Read> {
	inner: R,
	peek: Option<u8>,
}

impl<R: io::Read> ByteReader<R> {
	fn new(inner: R) -> io::Result<Self> {
		let mut v = Self { inner, peek: None };
		assert_eq!(None, v.get()?);
		Ok(v)
	}
}
impl<R: io::Read> ByteReader<R> {
	fn get(&mut self) -> io::Result<Option<u8>> {
		let b = self.get_without_peek()?;
		let v = self.peek;
		self.peek = b;
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
		self.peek.clone()
	}
}

#[derive(Debug)]
pub struct FormReaderError {
	message: String,
	cause: Option<Box<dyn Error>>,
}

impl Display for FormReaderError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("FormReaderError: {}", self.message))
	}
}
impl Error for FormReaderError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.cause.as_deref()
	}
}
impl From<io::Error> for FormReaderError {
	fn from(value: io::Error) -> Self {
		FormReaderError {
			message: String::from("IO error occurred"),
			cause: Some(Box::new(value)),
		}
	}
}
impl From<std::string::FromUtf8Error> for FormReaderError {
	fn from(value: std::string::FromUtf8Error) -> Self {
		FormReaderError {
			message: String::from("Invalid UTF-8 input received"),
			cause: Some(Box::new(value)),
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
				assert_eq!(r.get().unwrap(), Some(String::from(x)));
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
}
