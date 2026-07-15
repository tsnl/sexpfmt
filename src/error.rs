use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SexpfmtError {
	#[error("IO error: {source}")]
	Io {
		#[from]
		source: std::io::Error,
	},

	#[error("Invalid UTF-8 at {position}")]
	InvalidUtf8 { position: Loc },

	#[error("Parse error at {position}: {message}")]
	Parse { message: String, position: Loc },

	#[error(
		"Mismatched bookends at {position}: got {got:?}, expected {expected:?} (opened at {opened_at})"
	)]
	MismatchedBookends {
		/// Location of the offending closing bookend.
		position: Loc,
		/// Style of the closing bookend that was found.
		got: crate::SExpBookendStyle,
		/// Style of the innermost open bookend (i.e. what should be closed).
		expected: crate::SExpBookendStyle,
		/// Location of the unmatched opening bookend.
		opened_at: Loc,
	},

	#[error("Unexpected EOF at {position}: {unclosed_count} unclosed bookends")]
	UnexpectedEof {
		/// Location of the innermost unclosed opening bookend.
		position: Loc,
		unclosed_count: usize,
	},
}

impl SexpfmtError {
	pub fn invalid_utf8(position: Loc) -> Self {
		Self::InvalidUtf8 { position }
	}

	pub fn parse_error<S: Into<String>>(message: S, position: Loc) -> Self {
		Self::Parse {
			message: message.into(),
			position,
		}
	}

	pub fn mismatched_bookends(
		position: Loc,
		got: crate::SExpBookendStyle,
		expected: crate::SExpBookendStyle,
		opened_at: Loc,
	) -> Self {
		Self::MismatchedBookends {
			position,
			got,
			expected,
			opened_at,
		}
	}

	pub fn unexpected_eof(position: Loc, unclosed_count: usize) -> Self {
		Self::UnexpectedEof {
			position,
			unclosed_count,
		}
	}
}

/// Convenience type alias.
pub type Result<T> = std::result::Result<T, SexpfmtError>;

/// A position in the input stream, used for error messages.
///
/// `offset` counts *bytes*; `line` and `column` count characters (both
/// 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Loc {
	offset: usize,
	line: usize,
	column: usize,
}

impl Loc {
	pub fn new(offset: usize, line: usize, column: usize) -> Self {
		Self {
			offset,
			line,
			column,
		}
	}

	pub fn offset(self) -> usize {
		self.offset
	}

	pub fn line(self) -> usize {
		self.line
	}

	pub fn column(self) -> usize {
		self.column
	}

	/// The location immediately after `c`, when `self` is the location of `c`.
	pub(crate) fn advanced_by(self, c: char) -> Self {
		let offset = self.offset + c.len_utf8();
		if c == '\n' {
			Self::new(offset, self.line + 1, 1)
		} else {
			Self::new(offset, self.line, self.column + 1)
		}
	}
}

impl fmt::Display for Loc {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"line {}, column {} (offset {})",
			self.line, self.column, self.offset
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::SExpBookendStyle;

	#[test]
	fn test_io_error_conversion() {
		let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
		let sexp_err: SexpfmtError = io_err.into();

		match sexp_err {
			SexpfmtError::Io { source } => {
				assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
			}
			_ => panic!("Expected Io error"),
		}
	}

	#[test]
	fn test_invalid_utf8_error() {
		let err = SexpfmtError::invalid_utf8(Loc::new(3, 1, 4));
		assert_eq!(
			format!("{err}"),
			"Invalid UTF-8 at line 1, column 4 (offset 3)"
		);
	}

	#[test]
	fn test_parse_error_with_position() {
		let err = SexpfmtError::parse_error("Unexpected token", Loc::new(10, 2, 5));
		assert_eq!(
			format!("{err}"),
			"Parse error at line 2, column 5 (offset 10): Unexpected token"
		);
	}

	#[test]
	fn test_mismatched_bookends_error() {
		let err = SexpfmtError::mismatched_bookends(
			Loc::new(5, 1, 6),
			SExpBookendStyle::SquareBrackets,
			SExpBookendStyle::Parentheses,
			Loc::new(0, 1, 1),
		);

		let display_str = format!("{err}");
		assert!(display_str.contains("Mismatched bookends at line 1, column 6 (offset 5)"));
		assert!(display_str.contains("got SquareBrackets"));
		assert!(display_str.contains("expected Parentheses"));
		assert!(display_str.contains("opened at line 1, column 1 (offset 0)"));
	}

	#[test]
	fn test_unexpected_eof_error() {
		let err = SexpfmtError::unexpected_eof(Loc::new(100, 5, 1), 3);
		assert_eq!(
			format!("{err}"),
			"Unexpected EOF at line 5, column 1 (offset 100): 3 unclosed bookends"
		);
	}

	#[test]
	fn test_line_col_display() {
		assert_eq!(
			format!("{}", Loc::new(123, 10, 5)),
			"line 10, column 5 (offset 123)"
		);
	}

	#[test]
	fn test_error_conversion_from_std_errors() {
		fn io_operation() -> Result<()> {
			std::fs::read_to_string("nonexistent_file.txt")?;
			Ok(())
		}

		match io_operation() {
			Err(SexpfmtError::Io { source: _ }) => {}
			other => panic!("Expected automatic conversion to SexpfmtError::Io, got {other:?}"),
		}
	}

	#[test]
	fn test_loc_advanced_by() {
		let loc = Loc::new(0, 1, 1);
		let loc = loc.advanced_by('a');
		assert_eq!(loc, Loc::new(1, 1, 2));
		let loc = loc.advanced_by('é'); // 2 bytes, 1 column
		assert_eq!(loc, Loc::new(3, 1, 3));
		let loc = loc.advanced_by('\n');
		assert_eq!(loc, Loc::new(4, 2, 1));
	}
}
