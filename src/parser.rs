use crate::reader::CharReader;
use crate::{Loc, Result, SExp, SExpBookendStyle, SexpfmtError};
use std::io;

/// A streaming S-expression parser.
///
/// Reads top-level S-expressions one at a time from any [`io::Read`] (input is
/// buffered internally), so a long stream can be processed without holding
/// more than one top-level form in memory. Lists are parsed recursively, with
/// no artificial nesting-depth limit.
///
/// The surface syntax is a simplified, LISP-like language:
///
/// - Lists are delimited by `( )`, `[ ]`, or `{ }`; bookends must match.
/// - `;` starts a line comment. Comments are consumed and discarded by
///   default, or preserved as [`SExp::Comment`] nodes when
///   [`ParserConfig::preserve_comments`] is set.
/// - String literal atoms are delimited by `"` and follow R7RS Scheme escape
///   rules: `\a \b \t \n \r \" \\ \|`, inline hex escapes `\x<hex>;`, and
///   line continuations (`\` + intraline whitespace + newline). Any other
///   character — including literal newlines, brackets, `;`, and NUL — is
///   preserved as-is. Atoms store their source text verbatim: escapes are
///   validated but not decoded.
/// - Any other run of non-whitespace, non-delimiter characters is a bare atom.
pub struct Parser<R: io::Read> {
	chars: CharReader<R>,
	config: ParserConfig,
}

/// Parsing options for [`Parser`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParserConfig {
	/// Preserve `;` line comments as [`SExp::Comment`] nodes instead of
	/// discarding them. Off by default.
	pub preserve_comments: bool,
}

impl<R: io::Read> Parser<R> {
	/// A parser with the default [`ParserConfig`] (comments are discarded).
	pub fn new(inner: R) -> Self {
		Self::with_config(inner, ParserConfig::default())
	}

	pub fn with_config(inner: R, config: ParserConfig) -> Self {
		Self {
			chars: CharReader::new(inner),
			config,
		}
	}

	/// Parse and return the next top-level S-expression, or `None` at EOF.
	///
	/// When [`ParserConfig::preserve_comments`] is set, a top-level comment is
	/// returned as its own [`SExp::Comment`].
	pub fn next_sexp(&mut self) -> Result<Option<SExp>> {
		if let Some(text) = self.skip_to_content()? {
			return Ok(Some(SExp::Comment(text)));
		}
		let loc = self.chars.loc();
		match self.chars.peek()? {
			None => Ok(None),
			Some(c) if is_close_bookend(c) => Err(SexpfmtError::parse_error(
				format!("unexpected closing bookend `{c}`"),
				loc,
			)),
			Some(c) => Ok(Some(self.parse_sexp(c, 1)?)),
		}
	}

	/// Parse a single S-expression starting at the peeked character `c`, which
	/// must not be whitespace, a comment, or a closing bookend. `depth` is the
	/// number of enclosing lists, including the one this call would open.
	fn parse_sexp(&mut self, c: char, depth: usize) -> Result<SExp> {
		match c {
			c if is_open_bookend(c) => self.parse_list(depth),
			'"' => self.scan_string_literal_atom(),
			_ => self.scan_bare_atom(),
		}
	}

	/// Parse a list whose opening bookend is the next character.
	fn parse_list(&mut self, depth: usize) -> Result<SExp> {
		let opened_at = self.chars.loc();
		let open = self
			.chars
			.next()?
			.expect("caller peeked an opening bookend");
		let style = bookend_of_open(open);
		let mut elems = Vec::new();
		loop {
			if let Some(text) = self.skip_to_content()? {
				elems.push(SExp::Comment(text));
				continue;
			}
			let loc = self.chars.loc();
			match self.chars.peek()? {
				None => return Err(SexpfmtError::unexpected_eof(opened_at, depth)),
				Some(c) if is_close_bookend(c) => {
					self.chars.next()?;
					let got = bookend_of_close(c);
					if got != style {
						return Err(SexpfmtError::mismatched_bookends(
							loc, got, style, opened_at,
						));
					}
					return Ok(if elems.is_empty() {
						SExp::Null(style)
					} else {
						SExp::List(elems, style)
					});
				}
				Some(c) => {
					let elem = self.parse_sexp(c, depth + 1)?;
					elems.push(elem);
				}
			}
		}
	}

	/// Skip whitespace and `;` line comments until the next token (or EOF).
	///
	/// When [`ParserConfig::preserve_comments`] is set, stops at the first
	/// comment and returns its text instead of discarding it.
	fn skip_to_content(&mut self) -> Result<Option<String>> {
		loop {
			match self.chars.peek()? {
				Some(c) if c.is_whitespace() => {
					self.chars.next()?;
				}
				Some(';') => {
					let text = self.scan_comment_text()?;
					if self.config.preserve_comments {
						return Ok(Some(text));
					}
				}
				_ => return Ok(None),
			}
		}
	}

	/// Consume a `;` comment through its terminating newline (or EOF) and
	/// return the text between them. A trailing `\r` (from a CRLF line ending)
	/// is not considered part of the text.
	fn scan_comment_text(&mut self) -> Result<String> {
		let semicolon = self.chars.next()?;
		debug_assert_eq!(semicolon, Some(';'));
		let mut text = String::new();
		loop {
			match self.chars.next()? {
				None | Some('\n') => break,
				Some(c) => text.push(c),
			}
		}
		if text.ends_with('\r') {
			text.pop();
		}
		Ok(text)
	}

	fn scan_bare_atom(&mut self) -> Result<SExp> {
		let mut text = String::new();
		while let Some(c) = self.chars.peek()? {
			if is_atom_terminator(c) {
				break;
			}
			text.push(c);
			self.chars.next()?;
		}
		debug_assert!(!text.is_empty());
		Ok(SExp::Atom(text))
	}

	/// Scan a string literal atom, preserving its source text verbatim
	/// (including the surrounding quotes and all escape sequences).
	fn scan_string_literal_atom(&mut self) -> Result<SExp> {
		let string_start = self.chars.loc();
		let mut text = String::new();
		let quote = self.chars.next()?.expect("caller peeked `\"`");
		text.push(quote);
		loop {
			let Some(c) = self.chars.next()? else {
				return Err(unterminated_string(string_start));
			};
			text.push(c);
			match c {
				'"' => return Ok(SExp::Atom(text)),
				'\\' => self.scan_escape_sequence(&mut text, string_start)?,
				_ => {}
			}
		}
	}

	/// Scan the remainder of an escape sequence; the leading `\` has already
	/// been consumed and appended to `text`.
	fn scan_escape_sequence(&mut self, text: &mut String, string_start: Loc) -> Result<()> {
		let escape_loc = self.chars.loc();
		let Some(c) = self.chars.next()? else {
			return Err(unterminated_string(string_start));
		};
		text.push(c);
		match c {
			'a' | 'b' | 't' | 'n' | 'r' | '"' | '\\' | '|' => Ok(()),
			'x' => self.scan_hex_escape_sequence(text, escape_loc),
			' ' | '\t' | '\n' | '\r' => self.scan_line_continuation(text, c, escape_loc, string_start),
			_ => Err(SexpfmtError::parse_error(
				format!("invalid escape sequence `\\{c}` in string literal"),
				escape_loc,
			)),
		}
	}

	/// Scan the remainder of an inline hex escape sequence
	/// `\x<hex scalar value>;`; the leading `\x` has already been consumed and
	/// appended to `text`.
	fn scan_hex_escape_sequence(&mut self, text: &mut String, escape_loc: Loc) -> Result<()> {
		let mut digits = String::new();
		loop {
			let Some(c) = self.chars.next()? else {
				return Err(SexpfmtError::parse_error(
					"unterminated `\\x...;` escape in string literal",
					escape_loc,
				));
			};
			text.push(c);
			match c {
				';' => break,
				c if c.is_ascii_hexdigit() => digits.push(c),
				_ => {
					return Err(SexpfmtError::parse_error(
						format!("invalid character `{c}` in `\\x...;` escape (expected a hex digit or `;`)"),
						escape_loc,
					));
				}
			}
		}
		if digits.is_empty() {
			return Err(SexpfmtError::parse_error(
				"empty `\\x...;` escape in string literal",
				escape_loc,
			));
		}
		let scalar = u32::from_str_radix(&digits, 16)
			.ok()
			.and_then(char::from_u32);
		if scalar.is_none() {
			return Err(SexpfmtError::parse_error(
				format!("`\\x{digits};` is not a valid Unicode scalar value"),
				escape_loc,
			));
		}
		Ok(())
	}

	/// Scan the remainder of a line continuation (`\` + intraline whitespace +
	/// line ending); the leading `\` and `first` have already been consumed and
	/// appended to `text`.
	///
	/// The intraline whitespace *after* the line ending needs no special
	/// handling because string text is preserved verbatim rather than decoded.
	fn scan_line_continuation(
		&mut self,
		text: &mut String,
		first: char,
		escape_loc: Loc,
		string_start: Loc,
	) -> Result<()> {
		let mut c = first;
		while c == ' ' || c == '\t' {
			let Some(next) = self.chars.next()? else {
				return Err(unterminated_string(string_start));
			};
			text.push(next);
			c = next;
		}
		match c {
			'\n' => Ok(()),
			'\r' => {
				if self.chars.peek()? == Some('\n') {
					let lf = self.chars.next()?.expect("just peeked");
					text.push(lf);
				}
				Ok(())
			}
			_ => Err(SexpfmtError::parse_error(
				"invalid escape sequence in string literal: `\\` followed by whitespace must continue onto the next line",
				escape_loc,
			)),
		}
	}
}

/// Parse every S-expression in `text` with the default [`ParserConfig`].
/// Convenience wrapper over [`Parser`].
pub fn parse_str(text: &str) -> Result<Vec<SExp>> {
	parse_str_with(text, ParserConfig::default())
}

/// Parse every S-expression in `text` with the given [`ParserConfig`].
/// Convenience wrapper over [`Parser`].
pub fn parse_str_with(text: &str, config: ParserConfig) -> Result<Vec<SExp>> {
	let mut parser = Parser::with_config(text.as_bytes(), config);
	let mut sexps = Vec::new();
	while let Some(sexp) = parser.next_sexp()? {
		sexps.push(sexp);
	}
	Ok(sexps)
}

fn unterminated_string(string_start: Loc) -> SexpfmtError {
	SexpfmtError::parse_error("unterminated string literal", string_start)
}

fn is_atom_terminator(c: char) -> bool {
	c.is_whitespace() || is_open_bookend(c) || is_close_bookend(c) || matches!(c, '"' | ';')
}

fn is_open_bookend(c: char) -> bool {
	matches!(c, '(' | '[' | '{')
}

fn is_close_bookend(c: char) -> bool {
	matches!(c, ')' | ']' | '}')
}

fn bookend_of_open(c: char) -> SExpBookendStyle {
	match c {
		'(' => SExpBookendStyle::Parentheses,
		'[' => SExpBookendStyle::SquareBrackets,
		'{' => SExpBookendStyle::CurlyBraces,
		_ => unreachable!("caller matched an opening bookend"),
	}
}

fn bookend_of_close(c: char) -> SExpBookendStyle {
	match c {
		')' => SExpBookendStyle::Parentheses,
		']' => SExpBookendStyle::SquareBrackets,
		'}' => SExpBookendStyle::CurlyBraces,
		_ => unreachable!("caller matched a closing bookend"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn atom(s: &str) -> SExp {
		SExp::Atom(s.to_string())
	}

	#[test]
	fn test_parse_atoms() {
		assert_eq!(
			parse_str("hello world").unwrap(),
			vec![atom("hello"), atom("world")]
		);
		assert_eq!(
			parse_str("1234 .567 123.9870").unwrap(),
			vec![atom("1234"), atom(".567"), atom("123.9870")]
		);
		assert_eq!(parse_str(r"#\space").unwrap(), vec![atom(r"#\space")]);
	}

	#[test]
	fn test_parse_string_atom_verbatim() {
		let s = r#""this is a string literal\ncomplete with \"escape sequences\"!""#;
		assert_eq!(parse_str(s).unwrap(), vec![atom(s)]);
	}

	#[test]
	fn test_parse_null() {
		assert_eq!(
			parse_str("()").unwrap(),
			vec![SExp::Null(SExpBookendStyle::Parentheses)]
		);
		assert_eq!(
			parse_str("[]").unwrap(),
			vec![SExp::Null(SExpBookendStyle::SquareBrackets)]
		);
		assert_eq!(
			parse_str("{}").unwrap(),
			vec![SExp::Null(SExpBookendStyle::CurlyBraces)]
		);
	}

	#[test]
	fn test_parse_lists() {
		let e = vec![atom("hello"), atom("world")];
		assert_eq!(
			parse_str("(hello world) [hello world] {hello world}").unwrap(),
			vec![
				SExp::List(e.clone(), SExpBookendStyle::Parentheses),
				SExp::List(e.clone(), SExpBookendStyle::SquareBrackets),
				SExp::List(e.clone(), SExpBookendStyle::CurlyBraces),
			]
		);
	}

	#[test]
	fn test_streaming_one_form_at_a_time() {
		let mut p = Parser::new("(a b)\n(c d)".as_bytes());
		assert_eq!(
			p.next_sexp().unwrap(),
			Some(SExp::List(
				vec![atom("a"), atom("b")],
				SExpBookendStyle::Parentheses
			))
		);
		assert_eq!(
			p.next_sexp().unwrap(),
			Some(SExp::List(
				vec![atom("c"), atom("d")],
				SExpBookendStyle::Parentheses
			))
		);
		assert_eq!(p.next_sexp().unwrap(), None);
		assert_eq!(p.next_sexp().unwrap(), None);
	}

	#[test]
	fn test_adjacent_forms_without_whitespace() {
		assert_eq!(
			parse_str(r#"a(b)"c""#).unwrap(),
			vec![
				atom("a"),
				SExp::List(vec![atom("b")], SExpBookendStyle::Parentheses),
				atom(r#""c""#),
			]
		);
	}

	// Regression: comments used to confuse the (whitespace-and-bracket-only)
	// form reader, leaking comment words into the output as atoms.
	#[test]
	fn test_top_level_comment_is_discarded() {
		assert_eq!(parse_str("; hello world\n").unwrap(), vec![]);
		assert_eq!(parse_str("; hello world").unwrap(), vec![]);
		assert_eq!(parse_str("; comment\nfoo").unwrap(), vec![atom("foo")]);
	}

	fn parse_preserving(text: &str) -> Result<Vec<SExp>> {
		parse_str_with(
			text,
			ParserConfig {
				preserve_comments: true,
			},
		)
	}

	fn comment(s: &str) -> SExp {
		SExp::Comment(s.to_string())
	}

	#[test]
	fn test_preserve_top_level_comments() {
		assert_eq!(
			parse_preserving("; hello\n(a) ; trailer").unwrap(),
			vec![
				comment(" hello"),
				SExp::List(vec![atom("a")], SExpBookendStyle::Parentheses),
				comment(" trailer"),
			]
		);
	}

	#[test]
	fn test_preserve_comments_inside_lists() {
		assert_eq!(
			parse_preserving("(a ; note with ) bracket\n b)").unwrap(),
			vec![SExp::List(
				vec![atom("a"), comment(" note with ) bracket"), atom("b")],
				SExpBookendStyle::Parentheses
			)]
		);
	}

	#[test]
	fn test_preserved_comment_text_is_verbatim() {
		// No newline at EOF, empty comment, and CRLF line endings.
		assert_eq!(parse_preserving(";tail").unwrap(), vec![comment("tail")]);
		assert_eq!(parse_preserving(";\n").unwrap(), vec![comment("")]);
		assert_eq!(
			parse_preserving("; a\r\nb").unwrap(),
			vec![comment(" a"), atom("b")]
		);
	}

	#[test]
	fn test_preserving_comments_does_not_split_strings() {
		let s = r#""not a ; comment""#;
		assert_eq!(parse_preserving(s).unwrap(), vec![atom(s)]);
	}

	// Regression: brackets inside comments used to break form splitting.
	#[test]
	fn test_comment_containing_brackets() {
		assert_eq!(
			parse_str("(a ; comment with ) bracket\n b)").unwrap(),
			vec![SExp::List(
				vec![atom("a"), atom("b")],
				SExpBookendStyle::Parentheses
			)]
		);
	}

	// Regression: brackets inside strings used to break form splitting.
	#[test]
	fn test_string_containing_brackets() {
		let s = r#"(name "a :) smiley")"#;
		assert_eq!(
			parse_str(s).unwrap(),
			vec![SExp::List(
				vec![atom("name"), atom(r#""a :) smiley""#)],
				SExpBookendStyle::Parentheses
			)]
		);
	}

	// Regression: top-level strings containing whitespace used to be split at
	// the whitespace.
	#[test]
	fn test_top_level_string_with_spaces() {
		assert_eq!(
			parse_str(r#""hello world""#).unwrap(),
			vec![atom(r#""hello world""#)]
		);
	}

	// Regression: `\\` used to be misparsed as an escaped quote.
	#[test]
	fn test_string_ending_with_escaped_backslash() {
		let s = r#"(x "a\\")"#;
		assert_eq!(
			parse_str(s).unwrap(),
			vec![SExp::List(
				vec![atom("x"), atom(r#""a\\""#)],
				SExpBookendStyle::Parentheses
			)]
		);
	}

	// Regression: NUL characters inside strings used to be silently dropped.
	#[test]
	fn test_string_preserves_nul() {
		assert_eq!(parse_str("\"x\0y\"").unwrap(), vec![atom("\"x\0y\"")]);
	}

	#[test]
	fn test_string_with_literal_newline() {
		assert_eq!(parse_str("\"a\nb\"").unwrap(), vec![atom("\"a\nb\"")]);
	}

	#[test]
	fn test_all_mnemonic_escapes() {
		for e in ["a", "b", "t", "n", "r", "\"", "\\", "|"] {
			let s = format!("\"x\\{e}y\"");
			assert_eq!(parse_str(&s).unwrap(), vec![atom(&s)], "escape \\{e}");
		}
	}

	#[test]
	fn test_hex_escapes() {
		for s in [r#""\x41;""#, r#""\x03bb;""#, r#""\x10FFFF;""#] {
			assert_eq!(parse_str(s).unwrap(), vec![atom(s)]);
		}
		for s in [
			r#""\x;""#,          // no digits
			r#""\xZZ;""#,        // not hex digits
			r#""\x41""#,         // unterminated (no `;`)
			r#""\xD800;""#,      // surrogate
			r#""\x110000;""#,    // beyond max scalar value
			r#""\xFFFFFFFFF;""#, // overflows u32
		] {
			assert!(
				matches!(parse_str(s), Err(SexpfmtError::Parse { .. })),
				"expected parse error for {s}"
			);
		}
	}

	#[test]
	fn test_line_continuation() {
		for s in [
			"\"a\\\nb\"",     // \ + LF
			"\"a\\ \t \nb\"", // \ + intraline whitespace + LF
			"\"a\\\r\nb\"",   // \ + CRLF
			"\"a\\\rb\"",     // \ + CR
			"\"a\\ \n  b\"",  // trailing intraline whitespace after newline
		] {
			assert_eq!(parse_str(s).unwrap(), vec![atom(s)], "input: {s:?}");
		}
		// `\` + whitespace that never reaches a newline is invalid.
		assert!(matches!(
			parse_str("\"a\\ b\""),
			Err(SexpfmtError::Parse { .. })
		));
	}

	#[test]
	fn test_invalid_escape() {
		match parse_str(r#""a\qb""#) {
			Err(SexpfmtError::Parse { message, position }) => {
				assert!(
					message.contains("invalid escape sequence `\\q`"),
					"{message}"
				);
				assert_eq!(position, Loc::new(3, 1, 4));
			}
			other => panic!("expected parse error, got {other:?}"),
		}
	}

	#[test]
	fn test_unterminated_string() {
		match parse_str("(a \"oops") {
			Err(SexpfmtError::Parse { message, position }) => {
				assert!(message.contains("unterminated string literal"), "{message}");
				assert_eq!(position, Loc::new(3, 1, 4)); // start of the string
			}
			other => panic!("expected parse error, got {other:?}"),
		}
	}

	#[test]
	fn test_mismatched_bookends() {
		match parse_str("(a]") {
			Err(SexpfmtError::MismatchedBookends {
				position,
				got,
				expected,
				opened_at,
			}) => {
				assert_eq!(position, Loc::new(2, 1, 3));
				assert_eq!(got, SExpBookendStyle::SquareBrackets);
				assert_eq!(expected, SExpBookendStyle::Parentheses);
				assert_eq!(opened_at, Loc::new(0, 1, 1));
			}
			other => panic!("expected MismatchedBookends, got {other:?}"),
		}
	}

	#[test]
	fn test_unexpected_closing_bookend() {
		match parse_str("a )") {
			// The parser is streaming: `a` parses fine, the stray `)` errors.
			Ok(_) => panic!("expected parse error"),
			Err(e) => {
				// parse_str stops at the first error, which occurs after `a`.
				assert!(matches!(e, SexpfmtError::Parse { .. }), "{e:?}");
			}
		}
		match parse_str(")") {
			Err(SexpfmtError::Parse { message, position }) => {
				assert!(
					message.contains("unexpected closing bookend `)`"),
					"{message}"
				);
				assert_eq!(position, Loc::new(0, 1, 1));
			}
			other => panic!("expected parse error, got {other:?}"),
		}
	}

	#[test]
	fn test_unexpected_eof_reports_innermost_open() {
		match parse_str("(a (b (c") {
			Err(SexpfmtError::UnexpectedEof {
				position,
				unclosed_count,
			}) => {
				assert_eq!(unclosed_count, 3);
				assert_eq!(position, Loc::new(6, 1, 7)); // the `(` before `c`
			}
			other => panic!("expected UnexpectedEof, got {other:?}"),
		}
	}

	#[test]
	fn test_invalid_utf8_input() {
		let mut p = Parser::new(&b"(a \xff)"[..]);
		assert!(matches!(
			p.next_sexp(),
			Err(SexpfmtError::InvalidUtf8 { position }) if position == Loc::new(3, 1, 4)
		));
	}

	#[test]
	fn test_unicode_atoms_and_columns() {
		assert_eq!(
			parse_str("(λ (x) x²)").unwrap(),
			vec![SExp::List(
				vec![
					atom("λ"),
					SExp::List(vec![atom("x")], SExpBookendStyle::Parentheses),
					atom("x²"),
				],
				SExpBookendStyle::Parentheses
			)]
		);
	}
}
