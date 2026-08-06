//! Format S-expressions in a consistent style that is both line-diffable and
//! human-readable.
//!
//! This crate powers the `sexpfmt` command-line tool, and exposes the same
//! functionality as a library. The two main entry points are [`format`] (any
//! [`io::Read`] to any [`io::Write`], streaming one top-level S-expression at
//! a time) and [`format_str`] (in-memory strings). Both are driven by a
//! [`Config`], whose fields mirror the CLI options one-to-one:
//!
//! ```
//! use sexpfmt::{Config, format_str};
//!
//! let config = Config::default();
//! assert_eq!(format_str("(a  b (c))", &config).unwrap(), "(a b (c))\n");
//! ```
//!
//! Non-default options, e.g. preserving comments (`--preserve-comments`) and
//! normalizing bookends (`--bookends parens`):
//!
//! ```
//! use sexpfmt::{Config, SExpBookendStyle, format_str};
//!
//! let mut config = Config::default();
//! config.parser.preserve_comments = true;
//! config.printer.bookends = Some(SExpBookendStyle::Parentheses);
//! let formatted = format_str("[x ; note\n y]", &config).unwrap();
//! assert_eq!(formatted, "(x\n  ; note\n  y)\n");
//! ```
//!
//! For finer control, the pipeline underneath is also public: [`Parser`]
//! yields [`SExp`] values, and [`write_sexp`] / [`sexp_to_string`] print them.
//!
//! A C API for embedding sexpfmt in other languages is available behind the
//! `capi` feature; see the `capi` module and `include/sexpfmt.h`.

mod error;
mod parser;
mod printer;
mod reader;
mod sexp;

#[cfg(feature = "capi")]
pub mod capi;

pub use error::*;
pub use parser::*;
pub use printer::*;
pub use sexp::*;

use std::io;

/// Options for [`format`] and [`format_str`]: the library-level counterpart
/// of the CLI options. Every CLI option that affects formatting has a field
/// here, split by the stage it applies to.
#[derive(Clone, Debug, Default)]
pub struct Config {
	/// Parsing options (`--preserve-comments`).
	pub parser: ParserConfig,
	/// Printing options (`--indent`, `--margin`, `--bookends`,
	/// `--pair-labels`).
	pub printer: PrinterConfig,
}

/// Read S-expressions from `input` and write them to `output`, formatted
/// according to `config`, one top-level S-expression at a time, each followed
/// by a newline.
///
/// The output is flushed after each top-level S-expression so that results
/// appear promptly when formatting a long-running stream.
pub fn format<R: io::Read, W: io::Write>(input: R, output: &mut W, config: &Config) -> Result<()> {
	let mut parser = Parser::with_config(input, config.parser);
	while let Some(sexp) = parser.next_sexp()? {
		write_sexp(output, &sexp, &config.printer)?;
		writeln!(output)?;
		output.flush()?;
	}
	Ok(())
}

/// Convenience wrapper over [`format`] for in-memory strings.
pub fn format_str(input: &str, config: &Config) -> Result<String> {
	let mut buf = Vec::new();
	format(input.as_bytes(), &mut buf, config)?;
	Ok(String::from_utf8(buf).expect("printer output is valid UTF-8"))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn fmt(s: &str) -> String {
		format_str(s, &Config::default()).unwrap()
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

	#[test]
	fn test_preserve_comments_end_to_end() {
		let mut config = Config::default();
		config.parser.preserve_comments = true;
		assert_eq!(
			format_str("; header\n(a b) ; trailer\n", &config).unwrap(),
			"; header\n(a b)\n; trailer\n"
		);
		assert_eq!(
			format_str("(a ; note\n b)", &config).unwrap(),
			"(a\n  ; note\n  b)\n"
		);
	}

	#[test]
	fn test_normalize_bookends_end_to_end() {
		let mut config = Config::default();
		config.printer.bookends = Some(SExpBookendStyle::SquareBrackets);
		assert_eq!(format_str("(a {b}) []", &config).unwrap(), "[a [b]]\n[]\n");
	}

	#[test]
	fn test_pair_labels_end_to_end() {
		let mut config = Config::default();
		config.printer.margin_width = 24;
		config.printer.pair_labels = true;
		assert_eq!(
			format_str(r#"(menu :version "0.1.2" :items (list a))"#, &config).unwrap(),
			"(menu\n  :version \"0.1.2\"\n  :items (list a))\n"
		);
	}
}
