# `sexpfmt`

[![CI checks](https://github.com/tsnl/sexpfmt/actions/workflows/ci.yml/badge.svg)](https://github.com/tsnl/sexpfmt/actions/workflows/ci.yml)
[![Publish](https://github.com/tsnl/sexpfmt/actions/workflows/publish.yml/badge.svg)](https://github.com/tsnl/sexpfmt/actions/workflows/publish.yml)

S-expressions are easy for machines to write, but generating formatted S-expressions can be painful.
`sexpfmt` formats an input stream in a consistent way such that the output is both line-diffable and human-readable.

The formatting style used by sexpfmt is highly regular, unlike what many Lispers and Schemers prefer. Each indentation
increments spaces by a fixed number of spaces (by default, 2).

```sexp
(object
  (object
    (name "croissant")
    (quantity 2))
  (object
    (name "latte")
    (quantity 1)
    (size "tall")))
```

## Input language

The S-expression data format used is highly simplified compared to LISP's:

- Lists are delimited by `( )`, `[ ]`, or `{ }`; bookends must match.
- `;` starts a line comment. Comments are currently discarded, not preserved.
- String literals are delimited by `"` and support the same escape sequences
  as R7RS Scheme: `\a`, `\b`, `\t`, `\n`, `\r`, `\"`, `\\`, `\|`, inline hex
  escapes (`\x41;`), and line continuations (a `\` at the end of a line).
  String contents — including brackets, `;`, and literal newlines — are
  preserved verbatim.
- Anything else is a bare atom.

There is no support for quote, quasiquote, unquote, or dot pair-builders.
The character literal `#\ ` (for space) is not supported either. Use `#\space` instead.
There is also no support for `#1234 = ...` expressions to construct graphs.

---

## Setup and Installation

- To build and install this tool, you will need `Cargo` and a Rust toolchain.
- Navigate to the root of this repository with a shell, then run:

  ```
  cargo install --path .
  ```
- To run tests, you will also need `bash`.

---

## Usage

```
Usage: sexpfmt [OPTIONS] [FILES]...

Arguments:
  [FILES]...  Input files, formatted to stdout in order; reads stdin if none
              are given. Pass `-` to read stdin explicitly

Options:
      --indent <INDENT>  Number of spaces per indentation level [default: 2]
      --margin <MARGIN>  Target maximum line width [default: 80]
  -h, --help             Print help
  -V, --version          Print version
```

Examples:

```bash
$ sexpfmt my-file.sexp > my-formatted-file.sexp
$ ./build/my-sexp-generator-program arg1 arg2 | sexpfmt >> formatted-logfile.sexp
$ sexpfmt --indent 4 --margin 100 < my-file.sexp
```

For examples of `sexpfmt`'s behavior, see the `test` directory.

---

## Releases

Release notes are published on the
[GitHub Releases](https://github.com/tsnl/sexpfmt/releases) page.

---

## TODO
- [ ] allow command line options to specify...
  - [x] whether to print help and exit (e.g. `-h` or `--help`)
  - [ ] whether to normalize bookend tokens
  - [x] the margin width and indent width.
  - [x] file input
- [ ] preserve comments when parsing.
- [ ] consider whether to support more features like quote, quasiquote, unquote, pair building, etc.
  - [ ] explicit support for labels, e.g. `(menu :version "0.1.2" :items (list ...))`
- [ ] better documentation
- [ ] C API, binaries for easier integration into expect-testing in other languages.
