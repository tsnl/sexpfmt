//! Property tests: printing then re-parsing any `SExp` must yield the same
//! `SExp` (round-trip), and formatting must be idempotent.

use proptest::prelude::*;
use sexpfmt::{PrinterConfig, SExp, SExpBookendStyle, parse_str, sexp_to_string};

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

fn arb_sexp() -> impl Strategy<Value = SExp> {
	let leaf = prop_oneof![
		arb_bare_atom().prop_map(SExp::Atom),
		arb_string_atom().prop_map(SExp::Atom),
		arb_style().prop_map(SExp::Null),
	];
	leaf.prop_recursive(5, 32, 6, |inner| {
		(proptest::collection::vec(inner, 1..6), arb_style())
			.prop_map(|(elems, style)| SExp::List(elems, style))
	})
}

proptest! {
	#[test]
	fn roundtrip(sexp in arb_sexp()) {
		let config = PrinterConfig::default();
		let printed = sexp_to_string(&sexp, &config);
		let reparsed = parse_str(&printed).unwrap();
		prop_assert_eq!(reparsed, vec![sexp]);
	}

	#[test]
	fn idempotent(sexp in arb_sexp(), indent in 1usize..8, margin in 1usize..120) {
		let config = PrinterConfig { indent_width: indent, margin_width: margin };
		let once = sexp_to_string(&sexp, &config);
		let reparsed = parse_str(&once).unwrap();
		prop_assert_eq!(reparsed.len(), 1);
		let twice = sexp_to_string(&reparsed[0], &config);
		prop_assert_eq!(once, twice);
	}
}
