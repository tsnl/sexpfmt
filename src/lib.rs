mod error;
mod parser;
mod printer;
mod reader;
mod sexp;

pub use error::*;
pub use parser::*;
pub use printer::*;
pub use sexp::*;

use std::io;

/// Read S-expressions from `input` and write them to `output`, formatted
/// according to `config`, one top-level S-expression at a time, each followed
/// by a newline.
///
/// The output is flushed after each top-level S-expression so that results
/// appear promptly when formatting a long-running stream.
pub fn format<R: io::Read, W: io::Write>(
	input: R,
	output: &mut W,
	config: &PrinterConfig,
) -> Result<()> {
	let mut parser = Parser::new(input);
	while let Some(sexp) = parser.next_sexp()? {
		write_sexp(output, &sexp, config)?;
		writeln!(output)?;
		output.flush()?;
	}
	Ok(())
}

/// Convenience wrapper over [`format`] for in-memory strings.
pub fn format_str(input: &str, config: &PrinterConfig) -> Result<String> {
	let mut buf = Vec::new();
	format(input.as_bytes(), &mut buf, config)?;
	Ok(String::from_utf8(buf).expect("printer output is valid UTF-8"))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn fmt(s: &str) -> String {
		format_str(s, &PrinterConfig::default()).unwrap()
	}

	#[test]
	fn test_format_simple_stream() {
		assert_eq!(fmt("(a  b)\n\n(c d)"), "(a b)\n(c d)\n");
	}

	#[test]
	fn test_format_empty_input() {
		assert_eq!(fmt(""), "");
		assert_eq!(fmt("  \n\t "), "");
	}

	// Regression: a top-level comment used to leak its words into the output
	// as atoms (with exit code 0).
	#[test]
	fn test_comment_only_input_produces_no_output() {
		assert_eq!(fmt("; hello world\n"), "");
	}

	#[test]
	fn test_comments_are_dropped_around_forms() {
		assert_eq!(fmt("; header\n(a b) ; trailer\n; footer"), "(a b)\n");
	}

	// Regression: adjacent top-level forms used to be printed with no
	// separator at all.
	#[test]
	fn test_adjacent_forms_print_one_per_line() {
		assert_eq!(fmt("a(b)"), "a\n(b)\n");
	}

	#[test]
	fn test_strings_and_comments_end_to_end() {
		assert_eq!(
			fmt("(msg \"not a comment: ; (\") ; not a string: \"\n()"),
			"(msg \"not a comment: ; (\")\n()\n"
		);
	}
}
