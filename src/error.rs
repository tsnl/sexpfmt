use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SexpfmtError {
	#[error("IO error: {source}")]
	Io {
		#[from]
		source: std::io::Error,
	},

	#[error("UTF-8 encoding error: {source}")]
	Utf8 {
		#[from]
		source: std::string::FromUtf8Error,
	},

	#[error("Form reader error{}: {message}", position.as_ref().map(|p| format!(" at {}", p)).unwrap_or_default())]
	FormReader {
		message: String,
		position: Option<Loc>,
		#[source]
		source: Option<Box<dyn std::error::Error + Send + Sync>>,
	},

	#[error("Parse error at {position}: {message}")]
	Parse {
		message: String,
		position: Loc,
		#[source]
		source: Option<Box<dyn std::error::Error + Send + Sync>>,
	},

	#[error("Mismatched bookends at {position}: got {got:?}, expected {expected:?}")]
	MismatchedBookends {
		position: Loc,
		got: crate::SExpBookendStyle,
		expected: crate::SExpBookendStyle,
	},

	#[error("Unexpected EOF at {position}: {unclosed_count} unclosed bookends")]
	UnexpectedEof {
		position: Loc,
		unclosed_count: usize,
	},

	#[error("Invalid input at {position}: {message}")]
	InvalidInput { message: String, position: Loc },
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

impl SexpfmtError {
	pub fn form_reader_error<S: Into<String>>(
		message: S,
		position: Option<Loc>,
		source: Option<Box<dyn std::error::Error + Send + Sync>>,
	) -> Self {
		Self::FormReader {
			message: message.into(),
			position,
			source,
		}
	}

	pub fn parse_error<S: Into<String>>(
		message: S,
		position: Loc,
		source: Option<Box<dyn std::error::Error + Send + Sync>>,
	) -> Self {
		Self::Parse {
			message: message.into(),
			position,
			source,
		}
	}

	pub fn mismatched_bookends(
		position: Loc,
		got: crate::SExpBookendStyle,
		expected: crate::SExpBookendStyle,
	) -> Self {
		Self::MismatchedBookends {
			position,
			got,
			expected,
		}
	}

	pub fn unexpected_eof(position: Loc, unclosed_count: usize) -> Self {
		Self::UnexpectedEof {
			position,
			unclosed_count,
		}
	}

	pub fn invalid_input<S: Into<String>>(message: S, position: Loc) -> Self {
		Self::InvalidInput {
			message: message.into(),
			position,
		}
	}
}

// Convenience type alias
pub type Result<T> = std::result::Result<T, SexpfmtError>;

// InputLoc is used for error messages:
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

	pub fn in_form(start_of_form_loc: Self, span: nom_locate::LocatedSpan<&str>) -> Self {
		// Calculate position within the form based on nom's position
		let offset_in_form = span.location_offset();
		let line_in_form = span.location_line() as usize;
		let column_in_form = span.get_column();

		// Adjust position to be relative to the original input
		Self {
			offset: start_of_form_loc.offset() + offset_in_form,
			line: start_of_form_loc.line() + line_in_form - 1,
			column: if line_in_form == 1 {
				start_of_form_loc.column() + column_in_form - 1
			} else {
				column_in_form
			},
		}
	}
}
