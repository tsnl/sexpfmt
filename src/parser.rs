use crate::reader::CharReader;
use crate::{Loc, Result, SExp, SExpBookendStyle, SexpfmtError};
use std::io;

/// Maximum bookend nesting depth accepted by [`Parser`]. Deeper input is
/// rejected with a parse error rather than risking a stack overflow further
/// down the pipeline.
pub const MAX_DEPTH: usize = 4096;

/// A streaming S-expression parser.
///
/// Reads top-level S-expressions one at a time from any [`io::Read`] (input is
/// buffered internally), so a long stream can be processed without holding
/// more than one top-level form in memory.
///
/// The surface syntax is a simplified, LISP-like language:
///
/// - Lists are delimited by `( )`, `[ ]`, or `{ }`; bookends must match.
/// - `;` starts a line comment; comments are consumed and discarded.
/// - String literal atoms are delimited by `"` and follow R7RS Scheme escape
///   rules: `\a \b \t \n \r \" \\ \|`, inline hex escapes `\x<hex>;`, and
///   line continuations (`\` + intraline whitespace + newline). Any other
///   character — including literal newlines, brackets, `;`, and NUL — is
///   preserved as-is. Atoms store their source text verbatim: escapes are
///   validated but not decoded.
/// - Any other run of non-whitespace, non-delimiter characters is a bare atom.
pub struct Parser<R: io::Read> {
	chars: CharReader<R>,
}

struct OpenList {
	style: SExpBookendStyle,
	opened_at: Loc,
	elems: Vec<SExp>,
}

impl<R: io::Read> Parser<R> {
	pub fn new(inner: R) -> Self {
		Self {
			chars: CharReader::new(inner),
		}
	}

	/// Parse and return the next top-level S-expression, or `None` at EOF.
	pub fn next_sexp(&mut self) -> Result<Option<SExp>> {
		let mut stack: Vec<OpenList> = Vec::new();
		loop {
			self.skip_trivia()?;
			let loc = self.chars.loc();
			let Some(c) = self.chars.peek()? else {
				return match stack.last() {
					None => Ok(None),
					Some(innermost) => Err(SexpfmtError::unexpected_eof(
						innermost.opened_at,
						stack.len(),
					)),
				};
			};
			let completed = match c {
				'(' | '[' | '{' => {
					self.chars.next()?;
					if stack.len() >= MAX_DEPTH {
						return Err(SexpfmtError::parse_error(
							format!("bookends nested deeper than {MAX_DEPTH} levels"),
							loc,
						));
					}
					stack.push(OpenList {
						style: bookend_of_open(c),
						opened_at: loc,
						elems: Vec::new(),
					});
					continue;
				}
				')' | ']' | '}' => {
					self.chars.next()?;
					let got = bookend_of_close(c);
					let Some(top) = stack.pop() else {
						return Err(SexpfmtError::parse_error(
							format!("unexpected closing bookend `{c}`"),
							loc,
						));
					};
					if top.style != got {
						return Err(SexpfmtError::mismatched_bookends(
							loc,
							got,
							top.style,
							top.opened_at,
						));
					}
					if top.elems.is_empty() {
						SExp::Null(top.style)
					} else {
						SExp::List(top.elems, top.style)
					}
				}
				'"' => self.scan_string()?,
				_ => self.scan_bare_atom()?,
			};
			match stack.last_mut() {
				Some(parent) => parent.elems.push(completed),
				None => return Ok(Some(completed)),
			}
		}
	}

	/// Skip whitespace and `;` line comments.
	fn skip_trivia(&mut self) -> Result<()> {
		loop {
			match self.chars.peek()? {
				Some(c) if c.is_whitespace() => {
					self.chars.next()?;
				}
				Some(';') => loop {
					match self.chars.next()? {
						None | Some('\n') => break,
						Some(_) => {}
					}
				},
				_ => return Ok(()),
			}
		}
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
	fn scan_string(&mut self) -> Result<SExp> {
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
				'\\' => self.scan_escape(&mut text, string_start)?,
				_ => {}
			}
		}
	}

	/// Scan the remainder of an escape sequence; the leading `\` has already
	/// been consumed and appended to `text`.
	fn scan_escape(&mut self, text: &mut String, string_start: Loc) -> Result<()> {
		let escape_loc = self.chars.loc();
		let Some(c) = self.chars.next()? else {
			return Err(unterminated_string(string_start));
		};
		text.push(c);
		match c {
			'a' | 'b' | 't' | 'n' | 'r' | '"' | '\\' | '|' => Ok(()),
			'x' => self.scan_hex_escape(text, escape_loc),
			' ' | '\t' | '\n' | '\r' => self.scan_line_continuation(text, c, escape_loc, string_start),
			_ => Err(SexpfmtError::parse_error(
				format!("invalid escape sequence `\\{c}` in string literal"),
				escape_loc,
			)),
		}
	}

	/// Scan the remainder of an inline hex escape `\x<hex scalar value>;`; the
	/// leading `\x` has already been consumed and appended to `text`.
	fn scan_hex_escape(&mut self, text: &mut String, escape_loc: Loc) -> Result<()> {
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

/// Parse every S-expression in `text`. Convenience wrapper over [`Parser`].
pub fn parse_str(text: &str) -> Result<Vec<SExp>> {
	let mut parser = Parser::new(text.as_bytes());
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
	c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '"' | ';')
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
	fn test_depth_limit() {
		let s = "(".repeat(MAX_DEPTH + 1);
		match parse_str(&s) {
			Err(SexpfmtError::Parse { message, .. }) => {
				assert!(message.contains("nested deeper"), "{message}");
			}
			other => panic!("expected parse error, got {other:?}"),
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
