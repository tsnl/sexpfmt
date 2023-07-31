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

pub fn parse_form(text: String) -> Vec<SExp> {
  let located_span = LocSpan::new(text.as_str());
  let res = file(located_span).finish().unwrap();
  assert!(res.0.is_empty());
  res.1
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
	map(alt((simple_atom, string_atom)), |x| SExp::Atom(x))(input)
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
fn line_comment<'a>(input: LocSpan<'a>) -> IResult<LocSpan> {
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
		assert_eq!(
			parse_form("; a simple message\nhello world".to_string()),
			vec![
				SExp::Atom("hello".to_string()),
				SExp::Atom("world".to_string()),
			]
		);
	}

	#[test]
	fn test_parse_atom_2() {
		assert_eq!(
			parse_form("1234 .567 123.9870".to_string()),
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
		assert_eq!(parse_form(s.to_string()), vec![SExp::Atom(s.to_string())]);
	}

  #[test]
  fn test_parse_atom_4() {
    let s = r#"#\space"#;
    assert_eq!(parse_form(s.to_string()), vec![SExp::Atom(s.to_string())]);
  }

	#[test]
	fn test_parse_atom_5() {
		assert_eq!(
			parse_form("()".into()),
			vec![SExp::Null(SExpBookendStyle::Parentheses)]
		);
		assert_eq!(
			parse_form("[]".into()),
			vec![SExp::Null(SExpBookendStyle::SquareBrackets)]
		);
		assert_eq!(
			parse_form("{}".into()),
			vec![SExp::Null(SExpBookendStyle::CurlyBraces)]
		);
	}

	#[test]
	fn test_parse_list_1() {
		let s = r#"(hello world) [hello world] {hello world}"#;
		let e = vec![SExp::Atom("hello".into()), SExp::Atom("world".into())];
		assert_eq!(
			parse_form(s.to_string()),
			vec![
				SExp::List(e.clone(), SExpBookendStyle::Parentheses),
				SExp::List(e.clone(), SExpBookendStyle::SquareBrackets),
				SExp::List(e.clone(), SExpBookendStyle::CurlyBraces),
			]
		);
	}
}
