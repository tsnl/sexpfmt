use super::*;

use nom::branch::*;
use nom::bytes::complete::*;
use nom::character::complete::*;
use nom::combinator::*;
use nom::multi::*;
use nom::sequence::*;
use nom::Finish;

use nom_locate::LocatedSpan;

type LocSpan<'a> = LocatedSpan<&'a str>;
type IResult<'a, T> = nom::IResult<LocSpan<'a>, T>;

pub fn parse_form(text: String, form_position: Loc) -> Result<Vec<SExp>> {
	let located_span = LocSpan::new(text.as_str());
	let res = file(located_span).finish().map_err(|e| {
		// Convert nom error to our error type with position information
		let span = e.input;
		// Calculate position within the form based on nom's position
		let offset_in_form = span.location_offset();
		let line_in_form = span.location_line() as usize;
		let column_in_form = span.get_column();

		// Adjust position to be relative to the original input
		let error_position = Loc::new(
			form_position.offset() + offset_in_form,
			form_position.line() + line_in_form - 1,
			if line_in_form == 1 {
				form_position.column() + column_in_form - 1
			} else {
				column_in_form
			},
		);

		SexpfmtError::parse_error(format!("Parse error: {:?}", e.code), error_position, None)
	})?;

	if !res.0.is_empty() {
		let remaining_span = res.0;
		let offset_in_form = remaining_span.location_offset();
		let line_in_form = remaining_span.location_line() as usize;
		let column_in_form = remaining_span.get_column();

		let error_position = Loc::new(
			form_position.offset() + offset_in_form,
			form_position.line() + line_in_form - 1,
			if line_in_form == 1 {
				form_position.column() + column_in_form - 1
			} else {
				column_in_form
			},
		);

		return Err(SexpfmtError::parse_error(
			format!("Unexpected input: '{}'", remaining_span.fragment()),
			error_position,
			None,
		));
	}

	Ok(res.1)
}

fn file(input: LocSpan) -> IResult<Vec<SExp>> {
	map(
		tuple((many0(sexp_with_opt_prefix_space), opt(nonempty_skip))),
		|(elems, _)| elems,
	)(input)
}

fn sexp(input: LocSpan) -> IResult<SExp> {
	alt((list, atom))(input)
}
fn sexp_with_opt_prefix_space(input: LocSpan) -> IResult<SExp> {
	map(tuple((opt(nonempty_skip), sexp)), |(_, t)| t)(input)
}

fn list(input: LocSpan) -> IResult<SExp> {
	alt((
		basic_list('(', ')', SExpBookendStyle::Parentheses),
		basic_list('[', ']', SExpBookendStyle::SquareBrackets),
		basic_list('{', '}', SExpBookendStyle::CurlyBraces),
	))(input)
}
fn basic_list(
	lp: char,
	rp: char,
	sexp_bookend_style: SExpBookendStyle,
) -> impl FnMut(LocSpan) -> IResult<SExp> {
	move |input| {
		map(
			tuple((char(lp), opt(sexp_seq), char(rp))),
			|(lp, res, _)| match res {
				Some(terms) => SExp::List(terms, sexp_bookend_style),
				None => SExp::Null(match lp {
					'(' => SExpBookendStyle::Parentheses,
					'[' => SExpBookendStyle::SquareBrackets,
					'{' => SExpBookendStyle::CurlyBraces,
					_ => panic!("invalid bookend in 'null'"),
				}),
			},
		)(input)
	}
}
fn sexp_seq(input: LocSpan) -> IResult<Vec<SExp>> {
	map(
		tuple((many1(sexp_with_opt_prefix_space), opt(nonempty_skip))),
		|(terms, _)| terms,
	)(input)
}

fn atom(input: LocSpan) -> IResult<SExp> {
	map(alt((simple_atom, string_atom)), SExp::Atom)(input)
}
fn simple_atom(input: LocSpan) -> IResult<String> {
	map(many1(none_of("\"\n\r\t ()[]{};")), |x| {
		x.into_iter().collect()
	})(input)
}
fn string_atom(input: LocSpan) -> IResult<String> {
	map(
		tuple((char('"'), many0(string_element), char('"'))),
		|(_, content, _)| {
			let mut chars = Vec::with_capacity(2 + content.len());
			chars.push('"');
			for substr in content {
				for c in substr.into_iter() {
					if c == '\0' {
						break;
					};
					chars.push(c);
				}
			}
			chars.push('"');
			chars.into_iter().collect()
		},
	)(input)
}
fn string_element(input: LocSpan) -> IResult<[char; 2]> {
	alt((string_escape_element, string_non_escape_element))(input)
}
fn string_non_escape_element(input: LocSpan) -> IResult<[char; 2]> {
	map(satisfy(|c| (c != '"' && c != '\n' && c != '\r')), |c| {
		[c, '\0']
	})(input)
}
fn string_escape_element(input: LocSpan) -> IResult<[char; 2]> {
	map(tag("\\\""), |_| ['\\', '"'])(input)
}

pub fn nonempty_skip(input: LocSpan) -> IResult<()> {
	map(many1(alt((multispace1, line_comment))), |_| ())(input)
}
fn line_comment(input: LocSpan) -> IResult<LocSpan> {
	map(
		tuple((char(';'), take_till(|c| c == '\n' || c == '\r'))),
		|pair: (char, LocSpan)| {
			let (_, c) = pair;
			c
		},
	)(input)
}

#[cfg(test)]
mod tests {
	// FIXME: instead of testing the 'parse_form' function, test individual
	// parser elements instead.

	use super::*;

	#[test]
	fn test_parse_atom_1() {
		let position = Loc::new(0, 1, 1);
		assert_eq!(
			parse_form("; a simple message\nhello world".to_string(), position).unwrap(),
			vec![
				SExp::Atom("hello".to_string()),
				SExp::Atom("world".to_string()),
			]
		);
	}

	#[test]
	fn test_parse_atom_2() {
		let position = Loc::new(0, 1, 1);
		assert_eq!(
			parse_form("1234 .567 123.9870".to_string(), position).unwrap(),
			vec![
				SExp::Atom("1234".to_string()),
				SExp::Atom(".567".to_string()),
				SExp::Atom("123.9870".to_string()),
			]
		);
	}

	#[test]
	fn test_parse_atom_3() {
		let s = r#""this is a string literal\ncomplete with \"escape sequences\"!""#;
		let position = Loc::new(0, 1, 1);
		assert_eq!(
			parse_form(s.to_string(), position).unwrap(),
			vec![SExp::Atom(s.to_string())]
		);
	}

	#[test]
	fn test_parse_atom_4() {
		let s = r#"#\space"#;
		let position = Loc::new(0, 1, 1);
		assert_eq!(
			parse_form(s.to_string(), position).unwrap(),
			vec![SExp::Atom(s.to_string())]
		);
	}

	#[test]
	fn test_parse_atom_5() {
		let position = Loc::new(0, 1, 1);
		assert_eq!(
			parse_form("()".into(), position.clone()).unwrap(),
			vec![SExp::Null(SExpBookendStyle::Parentheses)]
		);
		assert_eq!(
			parse_form("[]".into(), position.clone()).unwrap(),
			vec![SExp::Null(SExpBookendStyle::SquareBrackets)]
		);
		assert_eq!(
			parse_form("{}".into(), position.clone()).unwrap(),
			vec![SExp::Null(SExpBookendStyle::CurlyBraces)]
		);
	}

	#[test]
	fn test_parse_list_1() {
		let s = r#"(hello world) [hello world] {hello world}"#;
		let e = vec![SExp::Atom("hello".into()), SExp::Atom("world".into())];
		let position = Loc::new(0, 1, 1);
		assert_eq!(
			parse_form(s.to_string(), position).unwrap(),
			vec![
				SExp::List(e.clone(), SExpBookendStyle::Parentheses),
				SExp::List(e.clone(), SExpBookendStyle::SquareBrackets),
				SExp::List(e.clone(), SExpBookendStyle::CurlyBraces),
			]
		);
	}

	#[test]
	fn test_parse_error_with_location() {
		let position = Loc::new(10, 2, 5);

		// Test invalid syntax that should produce a parse error
		let result = parse_form("(hello &invalid".to_string(), position);
		assert!(result.is_err());

		if let Err(SexpfmtError::Parse {
			message: _,
			position: error_pos,
			source: _,
		}) = result
		{
			// Error should be at or near the original position
			assert!(error_pos.line() >= 2);
			assert!(error_pos.offset() >= 10);
		} else {
			panic!("Expected Parse error, got: {:?}", result);
		}
	}
}
