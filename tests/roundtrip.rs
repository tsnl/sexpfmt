//! Property tests: printing then re-parsing any `SExp` must yield the same
//! `SExp` (round-trip), and formatting must be idempotent.

use proptest::prelude::*;
use sexpfmt::{
	ParserConfig, PrinterConfig, SExp, SExpBookendStyle, parse_str_with, sexp_to_string,
};

/// Parsing config for re-reading printed output: comments must be preserved,
/// or trees containing [`SExp::Comment`] cannot round-trip.
const REPARSE: ParserConfig = ParserConfig {
	preserve_comments: true,
};

fn arb_style() -> impl Strategy<Value = SExpBookendStyle> {
	prop_oneof![
		Just(SExpBookendStyle::Parentheses),
		Just(SExpBookendStyle::SquareBrackets),
		Just(SExpBookendStyle::CurlyBraces),
	]
}

/// Bare atoms: characters that never collide with delimiters, comments, or
/// string quoting.
fn arb_bare_atom() -> impl Strategy<Value = String> {
	"[A-Za-z0-9._+-]{1,12}"
}

/// Label atoms (`:` + at least one character): in a multi-line list these
/// share a line with the element that follows them, so generating them
/// exercises the label-pairing layout.
fn arb_label_atom() -> impl Strategy<Value = String> {
	":[A-Za-z0-9._+-]{1,10}"
}

/// String literal atoms, as verbatim source text (quotes included), built from
/// pieces that are each valid string-literal content: safe literal characters,
/// R7RS escapes, and characters that are special *outside* strings (brackets,
/// `;`, whitespace, newlines) — the parser must preserve all of them.
fn arb_string_atom() -> impl Strategy<Value = String> {
	let piece = prop_oneof![
		"[A-Za-z0-9 .,:!?+*/<=>@^_~$%&-]{1,8}".prop_map(|s| s.to_string()),
		Just("\\a".to_string()),
		Just("\\n".to_string()),
		Just("\\t".to_string()),
		Just("\\\\".to_string()),
		Just("\\\"".to_string()),
		Just("\\|".to_string()),
		Just("\\x41;".to_string()),
		Just("\\x03bb;".to_string()),
		Just("(".to_string()),
		Just(")".to_string()),
		Just("[".to_string()),
		Just("{".to_string()),
		Just(";".to_string()),
		Just("\n".to_string()),
	];
	proptest::collection::vec(piece, 0..6).prop_map(|pieces| format!("\"{}\"", pieces.concat()))
}

/// Comment text: any printable ASCII (comments swallow everything up to the
/// line ending, so brackets, quotes, and `;` are all fair game). Newlines are
/// impossible in comment text by construction, and `\r` is excluded because
/// the parser treats a trailing `\r` as part of a CRLF line ending.
fn arb_comment() -> impl Strategy<Value = String> {
	"[ -~]{0,16}"
}

fn arb_sexp() -> impl Strategy<Value = SExp> {
	let leaf = prop_oneof![
		arb_bare_atom().prop_map(SExp::Atom),
		arb_label_atom().prop_map(SExp::Atom),
		arb_string_atom().prop_map(SExp::Atom),
		arb_comment().prop_map(SExp::Comment),
		arb_style().prop_map(SExp::Null),
	];
	leaf.prop_recursive(5, 32, 6, |inner| {
		(proptest::collection::vec(inner, 1..6), arb_style())
			.prop_map(|(elems, style)| SExp::List(elems, style))
	})
}

fn arb_printer_config() -> impl Strategy<Value = PrinterConfig> {
	(1usize..8, 1usize..120, proptest::option::of(arb_style())).prop_map(
		|(indent_width, margin_width, bookends)| PrinterConfig {
			indent_width,
			margin_width,
			bookends,
		},
	)
}

proptest! {
	#[test]
	fn roundtrip(sexp in arb_sexp()) {
		let config = PrinterConfig::default();
		let printed = sexp_to_string(&sexp, &config);
		let reparsed = parse_str_with(&printed, REPARSE).unwrap();
		prop_assert_eq!(reparsed, vec![sexp]);
	}

	#[test]
	fn idempotent(sexp in arb_sexp(), config in arb_printer_config()) {
		let once = sexp_to_string(&sexp, &config);
		let reparsed = parse_str_with(&once, REPARSE).unwrap();
		prop_assert_eq!(reparsed.len(), 1);
		let twice = sexp_to_string(&reparsed[0], &config);
		prop_assert_eq!(once, twice);
	}
}
