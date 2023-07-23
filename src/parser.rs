use super::*;

use nom::Finish;
use nom::bytes::complete::*;
use nom::character::complete::*;
use nom::branch::*;
use nom::sequence::*;
use nom::combinator::*;
use nom::multi::*;

use nom_locate::LocatedSpan;

type LocSpan<'a> = LocatedSpan<&'a str>;
type IResult<'a, T> = nom::IResult<LocSpan<'a>, T>;

pub fn parse_file(text: String) -> Vec<SExp> {
  let located_span = LocSpan::new(text.as_str());
  let res = file(located_span).finish().unwrap();
  assert!(res.0.is_empty());
  res.1
}

fn file(input: LocSpan) -> IResult<Vec<SExp>> {
  sexp_seq(input)
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
fn basic_list(lp: char, rp: char, sexp_bookend_style: SExpBookendStyle) -> impl FnMut(LocSpan) -> IResult<SExp> {
  move |input| {
    map(
      tuple((char(lp), opt(sexp_seq), char(rp))),
      |(_, res, _)| 
        match res {
          Some(terms) => SExp::List(terms, sexp_bookend_style),
          None => SExp::Null,
        }
    )(input)
  }
}
fn sexp_seq(input: LocSpan) -> IResult<Vec<SExp>> {
  map(tuple((many1(sexp_with_opt_prefix_space), opt(nonempty_skip))), |(terms, _)| terms)(input)
}

fn atom(input: LocSpan) -> IResult<SExp> {
  map(alt((simple_atom, string_atom)), |x| SExp::Atom(x))(input)
}
fn simple_atom(input: LocSpan) -> IResult<String> {
  map(many1(none_of("\"\n\r\t ();")), |x| x.into_iter().collect())(input)
}
fn string_atom(input: LocSpan) -> IResult<String> {
  map(
    tuple((char('"'), many0(string_element), char('"'))),
    |(_, mut content, _)| {
      let mut chars = Vec::with_capacity(2 + content.len());
      chars.push('"');
      chars.append(&mut content);
      chars.push('"');
      chars.into_iter().collect()
    }
  )(input)
}
fn string_element(input: LocSpan) -> IResult<char> {
  alt((
    string_escape_element,
    string_non_escape_element
  ))(input)
}
fn string_non_escape_element(input: LocSpan) -> IResult<char> {
  satisfy(|c| (c != '"' && c != '\n' && c != '\r'))(input)
}
fn string_escape_element(input: LocSpan) -> IResult<char> {
  map(
    alt((tag("\\n"), tag("\\r"), tag("\\t"), tag("\\\\"), tag("\\\""))),
    |t: LocSpan| {
      match *t.fragment() {
        "\\n" => '\n',
        "\\r" => '\r',
        "\\t" => '\t',
        "\\\\" => '\\',
        "\\\"" => '\"',
        _ => panic!("invalid escape sequence: {}", t),
      }
    }
  )(input)
}

pub fn nonempty_skip(input: LocSpan) -> IResult<()> {
  map(
    many1(alt((multispace1, line_comment))),
    |_| ()
  )(input)
}
fn line_comment<'a>(input: LocSpan<'a>) -> IResult<LocSpan> {
  map(
    tuple((char(';'), take_till(|c| c == '\n' || c == '\r'))),
    |pair: (char, LocSpan)| {
      let (_, c) = pair;
      c
    }
  )(input)
}