#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SExp {
  List(Vec<SExp>, SExpBookendStyle),
  Atom(String),
  Null(SExpBookendStyle),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SExpBookendStyle {
  Parentheses,
  SquareBrackets,
  CurlyBraces,
}

