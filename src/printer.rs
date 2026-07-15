use crate::SExp;
use std::io;
use std::io::Write;
use unicode_width::UnicodeWidthStr;

/// Formatting options for [`write_sexp`].
#[derive(Clone, Debug)]
pub struct PrinterConfig {
	/// Number of spaces added per indentation level.
	pub indent_width: usize,
	/// Target maximum line width. Lists that fit within the margin are printed
	/// on a single line; lists that don't are broken with one element per line.
	/// Atoms are never broken, so a line can still exceed the margin.
	pub margin_width: usize,
}

impl Default for PrinterConfig {
	fn default() -> Self {
		Self {
			indent_width: 2,
			margin_width: 80,
		}
	}
}

/// Display width of `()`, `[]`, and `{}`.
const NULL_WIDTH: usize = 2;

enum PrintPlan {
	Null,
	Atom(usize),
	List(usize, Vec<PrintPlan>, ListPrintPlan),
}

enum ListPrintPlan {
	Monoline,
	Multiline,
}

impl PrintPlan {
	fn width(&self) -> usize {
		match self {
			PrintPlan::Null => NULL_WIDTH,
			PrintPlan::Atom(w) => *w,
			PrintPlan::List(w, _, _) => *w,
		}
	}
}

/// Write `sexp` to `w`, formatted according to `config`, without a trailing
/// newline.
///
/// # Panics
///
/// Panics if `sexp` contains an empty [`SExp::List`]: the empty list is
/// represented by [`SExp::Null`].
pub fn write_sexp<W: Write>(w: &mut W, sexp: &SExp, config: &PrinterConfig) -> io::Result<()> {
	let plan = plan(sexp, Some(config.margin_width), config);
	write_impl(w, sexp, &plan, 0, config)
}

/// Format `sexp` as a `String`, without a trailing newline. See [`write_sexp`].
pub fn sexp_to_string(sexp: &SExp, config: &PrinterConfig) -> String {
	let mut buf = Vec::new();
	write_sexp(&mut buf, sexp, config).expect("writing to a Vec cannot fail");
	String::from_utf8(buf).expect("printer output is valid UTF-8")
}

/// Display width of an atom. Atoms containing literal newlines (possible in
/// string literals) are measured by their widest line.
fn atom_width(text: &str) -> usize {
	text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0)
}

/// Compute a print plan for `sexp` in `available_width` columns.
/// `available_width == None` means unlimited width: the sexp is being planned
/// for a context that is already known to fit on a single line.
fn plan(sexp: &SExp, available_width: Option<usize>, config: &PrinterConfig) -> PrintPlan {
	match sexp {
		SExp::Null(_) => PrintPlan::Null,
		SExp::Atom(v) => PrintPlan::Atom(atom_width(v)),
		SExp::List(es, _) => {
			assert!(
				!es.is_empty(),
				"cannot print an empty SExp::List; the empty list is SExp::Null"
			);
			let elem_plans: Vec<PrintPlan> = es.iter().map(|x| plan(x, None, config)).collect();
			// `(elem elem ... elem)`: two bookends, elements, separating spaces.
			let monoline_width =
				2 + elem_plans.iter().map(PrintPlan::width).sum::<usize>() + (es.len() - 1);
			match available_width {
				None => PrintPlan::List(monoline_width, elem_plans, ListPrintPlan::Monoline),
				Some(available) if monoline_width <= available => {
					PrintPlan::List(monoline_width, elem_plans, ListPrintPlan::Monoline)
				}
				Some(available) => {
					let child_available = available.saturating_sub(config.indent_width);
					let ml_elem_plans: Vec<PrintPlan> = es
						.iter()
						.map(|x| plan(x, Some(child_available), config))
						.collect();
					let width = config.indent_width
						+ ml_elem_plans
							.iter()
							.map(PrintPlan::width)
							.max()
							.expect("list is non-empty")
						+ 1;
					PrintPlan::List(width, ml_elem_plans, ListPrintPlan::Multiline)
				}
			}
		}
	}
}

fn write_impl<W: Write>(
	w: &mut W,
	sexp: &SExp,
	plan: &PrintPlan,
	indent: usize,
	config: &PrinterConfig,
) -> io::Result<()> {
	match (sexp, plan) {
		(SExp::Null(bookend_style), PrintPlan::Null) => {
			write!(
				w,
				"{}{}",
				bookend_style.open_char(),
				bookend_style.close_char()
			)
		}
		(SExp::Atom(s), PrintPlan::Atom(_)) => write!(w, "{s}"),
		(SExp::List(es, bookend_style), PrintPlan::List(_, es_pps, linebreak)) => {
			assert!(
				!es.is_empty(),
				"cannot print an empty SExp::List; the empty list is SExp::Null"
			);
			// When the first element is itself a multi-line list, pad the
			// bookends with spaces so the head's own bookends stay visually
			// distinct from ours.
			let insert_padding_space = matches!(
				es_pps.first(),
				Some(PrintPlan::List(_, _, ListPrintPlan::Multiline))
			);
			write!(w, "{}", bookend_style.open_char())?;
			if insert_padding_space {
				write!(w, " ")?;
			}
			let child_indent = indent + config.indent_width;
			match linebreak {
				ListPrintPlan::Monoline => {
					for (i, (e, pp)) in es.iter().zip(es_pps).enumerate() {
						if i > 0 {
							write!(w, " ")?;
						}
						write_impl(w, e, pp, indent, config)?;
					}
				}
				ListPrintPlan::Multiline => {
					for (i, (e, pp)) in es.iter().zip(es_pps).enumerate() {
						if i > 0 {
							writeln!(w)?;
							write!(w, "{:child_indent$}", "")?;
						}
						write_impl(w, e, pp, child_indent, config)?;
					}
				}
			}
			if insert_padding_space {
				write!(w, " ")?;
			}
			write!(w, "{}", bookend_style.close_char())
		}
		_ => panic!("sexp-plan mismatch"),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{SExpBookendStyle, parse_str};

	fn fmt1(s: &str) -> String {
		fmt1_with(s, &PrinterConfig::default())
	}

	fn fmt1_with(s: &str, config: &PrinterConfig) -> String {
		let v = parse_str(s).unwrap();
		assert_eq!(v.len(), 1, "expected exactly one sexp in {s:?}");
		sexp_to_string(&v[0], config)
	}

	#[test]
	fn test_null_styles() {
		assert_eq!(fmt1("()"), "()");
		assert_eq!(fmt1("[]"), "[]");
		assert_eq!(fmt1("{}"), "{}");
	}

	#[test]
	fn test_atom_verbatim() {
		assert_eq!(fmt1("hello"), "hello");
		assert_eq!(fmt1(r#""a \x41; b""#), r#""a \x41; b""#);
	}

	#[test]
	fn test_monoline_list() {
		assert_eq!(fmt1("(a   b\n c)"), "(a b c)");
		assert_eq!(fmt1("[a {b} ()]"), "[a {b} ()]");
	}

	#[test]
	fn test_multiline_break() {
		let long = "(word word word word word word word word word word word word word word word word)";
		assert_eq!(
			fmt1(long),
			"(word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word\n  word)"
		);
	}

	#[test]
	fn test_custom_indent() {
		let config = PrinterConfig {
			indent_width: 4,
			margin_width: 10,
		};
		assert_eq!(
			fmt1_with("(aaaa bbbb cccc)", &config),
			"(aaaa\n    bbbb\n    cccc)"
		);
	}

	// Mirrors test/test002-multiline_head.sexp: a multi-line list in head
	// position gets padding spaces inside the outer bookends.
	#[test]
	fn test_multiline_head_padding() {
		let input = r#"(map ((list "avocado" "banana" "canteloupe" "durian" "eggplant" "fig" "grape" "habanero") 42))"#;
		let expected = r#"(map
  ( (list
      "avocado"
      "banana"
      "canteloupe"
      "durian"
      "eggplant"
      "fig"
      "grape"
      "habanero")
    42 ))"#;
		assert_eq!(fmt1(input), expected);
	}

	// Regression: `available_width` used to hit the "unlimited" sentinel value
	// of 0 at nesting depth `margin/indent`, printing arbitrarily wide lists on
	// a single line.
	#[test]
	fn test_deeply_nested_lists_still_break() {
		let inner_atoms: Vec<String> = (0..20).map(|i| format!("atom{i}")).collect();
		let mut s = format!("({})", inner_atoms.join(" "));
		for _ in 0..41 {
			s = format!("(x {s})");
		}
		let out = fmt1(&s);
		assert!(
			!out.contains("atom0 atom1"),
			"inner list should have been broken across lines:\n{out}"
		);
	}

	// A list whose display width fits the margin must stay on one line even
	// when its byte length exceeds the margin (byte length used to be used as
	// the width).
	#[test]
	fn test_width_is_display_width_not_byte_count() {
		let atom = "é".repeat(30); // 30 columns, 60 bytes
		let s = format!("({atom} {atom})"); // 63 columns, 123 bytes
		let out = fmt1(&s);
		assert!(!out.contains('\n'), "list should be monoline: {out}");
	}

	#[test]
	#[should_panic(expected = "empty SExp::List")]
	fn test_empty_list_panics() {
		sexp_to_string(
			&SExp::List(Vec::new(), SExpBookendStyle::Parentheses),
			&PrinterConfig::default(),
		);
	}

	#[test]
	#[should_panic(expected = "empty SExp::List")]
	fn test_nested_empty_list_panics() {
		sexp_to_string(
			&SExp::List(
				vec![SExp::List(Vec::new(), SExpBookendStyle::Parentheses)],
				SExpBookendStyle::Parentheses,
			),
			&PrinterConfig::default(),
		);
	}

	#[test]
	fn test_multiline_string_atom_width_uses_widest_line() {
		// The atom contains a literal newline; planning must not treat the
		// whole atom as one huge line.
		let s = "(a \"xx\nyy\" b)";
		assert_eq!(fmt1(s), "(a \"xx\nyy\" b)");
	}
}
