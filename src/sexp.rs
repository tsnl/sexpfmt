#[derive(Debug)]
pub enum SExp {
  List(Vec<SExp>, SExpBookendStyle),
  Atom(String),
  Null,
}

#[derive(Clone, Copy, Debug)]
pub enum SExpBookendStyle {
  Parentheses,
  SquareBrackets,
  CurlyBraces,
}
