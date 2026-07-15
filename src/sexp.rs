/// A parsed S-expression.
///
/// Atoms store their *verbatim source text*: a string literal atom includes its
/// surrounding double quotes and keeps its escape sequences exactly as written,
/// so printing an atom never loses information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SExp {
	/// A non-empty list. The empty list is represented by [`SExp::Null`];
	/// the printer panics on an empty `List`.
	List(Vec<SExp>, SExpBookendStyle),
	Atom(String),
	/// The empty list: `()`, `[]`, or `{}`.
	Null(SExpBookendStyle),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SExpBookendStyle {
	Parentheses,
	SquareBrackets,
	CurlyBraces,
}

impl SExpBookendStyle {
	pub fn open_char(self) -> char {
		match self {
			Self::Parentheses => '(',
			Self::SquareBrackets => '[',
			Self::CurlyBraces => '{',
		}
	}

	pub fn close_char(self) -> char {
		match self {
			Self::Parentheses => ')',
			Self::SquareBrackets => ']',
			Self::CurlyBraces => '}',
		}
	}
}
