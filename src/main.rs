mod parser;
mod printer;
mod sexp;

use parser::*;
use printer::*;
use sexp::*;

use std::io::{stdin, Read};

const READ_BUFFER_SIZE: usize = 512;
const FORM_BUFFER_CAPACITY: usize = 8192;
const PAREN_STACK_CAPACITY: usize = 64;

/// read_single_form extracts a single form
fn read_single_form(buf: &mut [u8], mut leftover_byte_count: usize) -> Option<(String, usize)> {
	enum LexerState {
		NotInForm,
		InAtomForm,
		InListForm,
	}

	let mut raw_form_bytes = Vec::with_capacity(FORM_BUFFER_CAPACITY);
	let mut paren_stack = Vec::with_capacity(PAREN_STACK_CAPACITY);
	let mut curr_state = LexerState::NotInForm;

	loop {
		// Reading a number of bytes into the buffer.
		// If there are leftover bytes from a previous read in the buffer, then use
		// these bytes first. Otherwise, replenish the buffer.
		let byte_count = {
			if leftover_byte_count > 0 {
				let old_byte_count = leftover_byte_count;
				leftover_byte_count = 0;
				old_byte_count
			} else {
				let fresh_byte_count = stdin().read(&mut buf[..]).unwrap();
				if fresh_byte_count == 0 {
					return None;
				}
				fresh_byte_count
			}
		};

		// Scanning the read buffer using the cursor, running a state machine.
		// NOTE: the only state transitions available are NotInForm -> {InAtomForm, InListForm}.
		// Instead of transitioning back to 'NotInForm', we just return the form string.

		let mut cursor = 0;

		macro_rules! return_form {
			() => {
				return Some((
					String::from_utf8(raw_form_bytes).unwrap(),
					byte_count - cursor - 1,
				));
			};
		}
		macro_rules! report_mismatched_bookends {
			() => {
				panic!("syntax error: bookends (parens, brackets, braces) mismatched.");
			};
		}
		macro_rules! push_bookend {
			($bookend_type:expr) => {
				raw_form_bytes.push(buf[cursor]);
				paren_stack.push($bookend_type);
			};
		}
		macro_rules! pop_bookend {
			($bookend_type:expr) => {
				raw_form_bytes.push(buf[cursor]);
				let actual_bookend_type = paren_stack.pop().unwrap();
				if actual_bookend_type != $bookend_type {
					report_mismatched_bookends!();
				};
				if paren_stack.len() == 0 {
					return_form!();
				}
			};
		}

		while cursor < byte_count {
			let next_state = match curr_state {
				LexerState::NotInForm => {
					if buf[cursor].is_ascii_whitespace() {
						curr_state
					} else {
						match buf[cursor] {
							b'(' => {
								push_bookend!(sexp::SExpBookendStyle::Parentheses);
								LexerState::InListForm
							}
							b'[' => {
								push_bookend!(sexp::SExpBookendStyle::SquareBrackets);
								LexerState::InListForm
							}
							b'{' => {
								push_bookend!(sexp::SExpBookendStyle::CurlyBraces);
								LexerState::InListForm
							}
							b')' | b']' | b'}' => {
								report_mismatched_bookends!();
							}
							_ => {
								raw_form_bytes.push(buf[cursor]);
								LexerState::InAtomForm
							}
						}
					}
				}
				LexerState::InAtomForm => {
					if buf[cursor].is_ascii_whitespace() {
						return_form!();
					} else {
						raw_form_bytes.push(buf[cursor]);
						curr_state
					}
				}
				LexerState::InListForm => match buf[cursor] {
					b'(' => {
						push_bookend!(sexp::SExpBookendStyle::Parentheses);
						curr_state
					}
					b'[' => {
						push_bookend!(sexp::SExpBookendStyle::SquareBrackets);
						curr_state
					}
					b'{' => {
						push_bookend!(sexp::SExpBookendStyle::CurlyBraces);
						curr_state
					}
					b')' => {
						pop_bookend!(sexp::SExpBookendStyle::Parentheses);
						curr_state
					}
					b']' => {
						pop_bookend!(sexp::SExpBookendStyle::SquareBrackets);
						curr_state
					}
					b'}' => {
						pop_bookend!(sexp::SExpBookendStyle::CurlyBraces);
						curr_state
					}
					b => {
						raw_form_bytes.push(b);
						curr_state
					}
				},
			};

			curr_state = next_state;
			cursor += 1;
		}
	}
}

fn main() {
	let mut buf = Box::new([0; READ_BUFFER_SIZE]);
	let mut leftover_byte_count = 0;
	loop {
		let (content, next_leftover_byte_count) = match read_single_form(&mut *buf, leftover_byte_count)
		{
			Some(v) => v,
			None => return,
		};

		let output = parse_form(content);
		print_sexp(output);
		println!();

		leftover_byte_count = next_leftover_byte_count;
	}
}
