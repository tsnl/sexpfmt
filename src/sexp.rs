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
	/// A `;` line comment, stored without the leading `;` or the line ending.
	///
	/// Only produced by the parser when
	/// [`ParserConfig::preserve_comments`](crate::ParserConfig::preserve_comments)
	/// is set; by default comments are discarded. The text must not contain a
	/// newline: the printer emits it as `;text` followed by a line break.
	Comment(String),
}

impl SExp {
	/// Whether this S-expression is, or contains, a [`SExp::Comment`].
	///
	/// Comments consume the rest of their line, so a list that contains one
	/// anywhere can never be printed on a single line.
	pub fn contains_comment(&self) -> bool {
		match self {
			SExp::Comment(_) => true,
			SExp::List(es, _) => es.iter().any(SExp::contains_comment),
			SExp::Atom(_) | SExp::Null(_) => false,
		}
	}
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
