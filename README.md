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
  Pass `--bookends <STYLE>` to normalize them all to one style.
- `;` starts a line comment. Comments are discarded by default; pass
  `--preserve-comments` to keep them in the output.
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
      --indent <INDENT>    Number of spaces per indentation level [default: 2]
      --margin <MARGIN>    Target maximum line width [default: 80]
      --bookends <STYLE>   Normalize all list bookends to the given style
                           instead of preserving each list's input style
                           [possible values: parens, square, curly]
      --preserve-comments  Preserve `;` line comments instead of discarding
                           them
      --pair-labels        In multi-line lists, keep a `:label` atom on the
                           same line as the element that follows it
  -h, --help               Print help (see more with '--help')
  -V, --version            Print version
```

Examples:

```bash
$ sexpfmt my-file.sexp > my-formatted-file.sexp
$ ./build/my-sexp-generator-program arg1 arg2 | sexpfmt >> formatted-logfile.sexp
$ sexpfmt --indent 4 --margin 100 < my-file.sexp
```

With `--pair-labels`, a `:label` atom shares a line with the element that
follows it when a list is broken across lines:

```sexp
(menu
  :version "0.1.2"
  :items (list ...))
```

For examples of `sexpfmt`'s behavior, see the `test` directory.

---

## Library usage (Rust)

`sexpfmt` is also a Rust library ([docs.rs/sexpfmt](https://docs.rs/sexpfmt)).
Every CLI option has a counterpart in `sexpfmt::Config`, so embedders get the
same behavior as the CLI:

```rust
use sexpfmt::{Config, format_str};

let mut config = Config::default();
config.parser.preserve_comments = true; // --preserve-comments
config.printer.indent_width = 4;        // --indent 4

let formatted = format_str("(a  b (c))", &config)?;
assert_eq!(formatted, "(a b (c))\n");
```

Use `sexpfmt::format` to stream from any `io::Read` to any `io::Write`, one
top-level S-expression at a time, or drive `Parser` / `write_sexp` directly.

---

## C API

For integration into expect-testing in other languages, a C API is available
behind the `capi` cargo feature. Build the library from the repository root:

```bash
cargo rustc --release --features capi --lib --crate-type cdylib    # shared
cargo rustc --release --features capi --lib --crate-type staticlib # static
```

The header lives at [`include/sexpfmt.h`](include/sexpfmt.h):

```c
#include "sexpfmt.h"

sexpfmt_config config = sexpfmt_config_default();
char *out = NULL, *error = NULL;
if (sexpfmt_format(input, input_len, &config, &out, NULL, &error) == SEXPFMT_OK) {
	fputs(out, stdout);
} else {
	fprintf(stderr, "sexpfmt: %s\n", error);
}
sexpfmt_str_free(out);
sexpfmt_str_free(error);
```

Alternatively, spawn the prebuilt `sexpfmt` binary attached to each
[GitHub release](https://github.com/tsnl/sexpfmt/releases) and pipe through
its stdin/stdout.

---

## Releases

Release notes are published on the
[GitHub Releases](https://github.com/tsnl/sexpfmt/releases) page, along with
prebuilt `sexpfmt` binaries for Linux (x86-64), macOS (arm64), and Windows
(x86-64).

---

## Design notes

`sexpfmt` deliberately does not support quote (`'x`), quasiquote/unquote
(`` `x ``, `,x`, `,@x`), or dot pair-builders (`(a . b)`). These are
conveniences for hand-written Lisp source, while `sexpfmt` targets
machine-generated S-expression *data*, where generators can (and should) emit
explicit `(quote x)`-style lists instead; keeping reader macros out keeps the
grammar small and every atom verbatim. If a concrete use case turns up, this
decision can be revisited.

---

## TODO
- [x] allow command line options to specify...
  - [x] whether to print help and exit (e.g. `-h` or `--help`)
  - [x] whether to normalize bookend tokens (`--bookends`)
  - [x] the margin width and indent width.
  - [x] file input
- [x] preserve comments when parsing (`--preserve-comments`).
- [x] consider whether to support more features like quote, quasiquote, unquote, pair building, etc.
      (decided against for now; see "Design notes" above)
  - [x] explicit support for labels, e.g. `(menu :version "0.1.2" :items (list ...))` (`--pair-labels`)
- [x] better documentation
- [x] C API, binaries for easier integration into expect-testing in other languages.
- [ ] attach comments to the element they follow, so `(a ; note` keeps the
      note on `a`'s line instead of its own.
