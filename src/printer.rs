// TODO: implement a printer using the following greedy algorithm:
// Each term is to be printed at an indentation level.
// We will recursively compute the length of a node in the tree such that one of the following holds true:
// - the entire list is placed on a single line, including each element list.
// - except for the first, each list entry is placed on a separate line with increased indentation level.
// When an intermediate element of a list is pushed to the next line, it does not matter if it exceeds the margin if it
// is an atom.
// NOTE: an awkward case is when the first element of a list is also a list that may be broken multi-line.
// E.g.
//   ((this is a long list)
//     arg1
//     arg2)
// We would break this as...
//   ( (this
//       is
//       a
//       long
//       list)
//     arg1
//     arg2 )
// Note the insertion of a leading and trailing space.

use super::*;

const NULL_TEXT: &str = "()";
const INDENT_WIDTH: i32 = 2;
const MARGIN_WIDTH: i32 = 80;

enum PrintPlan {
	Null,
	Atom(i32),
	List(i32, Vec<PrintPlan>, ListPrintPlan),
}
enum ListPrintPlan {
	Monoline,
	Multiline,
}
impl PrintPlan {
	fn width(&self) -> i32 {
		match self {
			PrintPlan::Null => NULL_TEXT.len().try_into().unwrap(),
			PrintPlan::Atom(w) => *w,
			PrintPlan::List(w, _, _) => *w,
		}
	}
}

pub fn print_sexp(sexp_vec: Vec<SExp>) {
	for sexp in sexp_vec.into_iter() {
		let print_plan = plan(&sexp, MARGIN_WIDTH);
		print_impl(sexp, print_plan, 0);
	}
}

fn plan(sexp: &SExp, available_width: i32) -> PrintPlan {
	match sexp {
		SExp::Null(_) => PrintPlan::Null,
		SExp::Atom(v) => {
			// cannot line-break atoms
			PrintPlan::Atom(v.len().try_into().unwrap())
		}
		SExp::List(es, _) => {
			let elem_plans: Vec<PrintPlan> = es.iter().map(|x| plan(x, 0)).collect();
			let monoline_width =
				1 + elem_plans.iter().map(|x| x.width()).sum::<i32>() + ((es.len() - 1) as i32) + 1;
			if available_width == 0 || monoline_width <= available_width {
				// can fit this entire list on a single line
				PrintPlan::List(monoline_width, elem_plans, ListPrintPlan::Monoline)
			} else {
				// multi-line:
				let ml_elem_plans: Vec<PrintPlan> = es
					.iter()
					.map(|x| plan(x, available_width - INDENT_WIDTH))
					.collect();
				let width = INDENT_WIDTH + ml_elem_plans.iter().map(|x| x.width()).max().unwrap() + 1;
				PrintPlan::List(width, ml_elem_plans, ListPrintPlan::Multiline)
			}
		}
	}
}

fn print_impl(sexp: SExp, plan: PrintPlan, indent: i32) {
	match (sexp, plan) {
		(SExp::Null(bookend_style), PrintPlan::Null) => {
			print!(
				"{}",
				match bookend_style {
					SExpBookendStyle::Parentheses => "()",
					SExpBookendStyle::CurlyBraces => "{}",
					SExpBookendStyle::SquareBrackets => "[]",
				}
			);
		}
		(SExp::Atom(s), PrintPlan::Atom(_)) => {
			print!("{}", s)
		}
		(SExp::List(es, bookend_style), PrintPlan::List(_, es_pps, linebreak)) => {
			let es_len = es.len();
			let insert_padding_space = if let PrintPlan::List(_, _, ListPrintPlan::Multiline) = es_pps[0]
			{
				// the first element is a multi-line list form itself.
				true
			} else {
				false
			};
			let (open_token, close_token) = match bookend_style {
				SExpBookendStyle::Parentheses => ('(', ')'),
				SExpBookendStyle::CurlyBraces => ('{', '}'),
				SExpBookendStyle::SquareBrackets => ('[', ']'),
			};

			print!("{}", open_token);
			if insert_padding_space {
				print!(" ");
			}
			match linebreak {
				ListPrintPlan::Monoline => {
					for (i, (e, pp)) in (0..es.len()).zip(es.into_iter().zip(es_pps.into_iter())) {
						print_impl(e, pp, indent);
						if i < es_len - 1 {
							print!(" ")
						}
					}
				}
				ListPrintPlan::Multiline => {
					for (i, (e, pp)) in (0..es.len()).zip(es.into_iter().zip(es_pps.into_iter())) {
						print_impl(e, pp, indent + INDENT_WIDTH);
						if i < es_len - 1 {
							println!();
							for _ in 0..(indent + INDENT_WIDTH) {
								print!(" ");
							}
						}
					}
				}
			}
			if insert_padding_space {
				print!(" ");
			}
			print!("{}", close_token);
		}
		_ => panic!("sexp-plan mismatch"),
	}
}
