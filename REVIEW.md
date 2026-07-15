# Code review: sexpfmt v1.0.1

Review of the full codebase (~1,100 lines of Rust) as of commit `a2c27c4`.
Every bug listed below was reproduced against a release build; repro commands
are included. The existing test suite (`./script/test.sh`) and
`cargo clippy` both pass, and formatting was verified to be idempotent on the
whole test corpus.

## Strengths

- Clean module layout (`reader` / `parser` / `printer` / `sexp` / `error`).
- Errors carry line/column/offset positions and a proper source chain via
  `thiserror`.
- The streaming one-form-at-a-time design with per-form flushing fits the
  log-pipeline use case described in the README.
- Snapshot test harness with auto-generated expectations is pleasant to use.
- Formatting is idempotent on the test corpus.
- CI and a parameterized publish workflow exist.

## The core problem: two disagreeing grammars

`FormReader` (`src/reader.rs:54`) splits the input stream into forms
understanding only *brackets and whitespace*, but the nom parser
(`src/parser.rs`) understands *strings and comments* too. Every place the two
disagree is a bug. Four confirmed:

### 1. Top-level comments silently leak into output as data

The worst bug of the batch, because it exits 0 with wrong output:

```
$ echo '; hello world' | sexpfmt
<blank line>
hello
world
```

The reader takes `;` as an atom-form, then emits `hello` and `world` as real
atoms. A comment becomes data.

### 2. Brackets inside strings break parsing

```
$ echo '(name "a :) smiley")' | sexpfmt
ERROR: Parse error at line 1, column 1 (offset 0): Unexpected input: '(name "a :)'
```

The reader counts the `)` inside the string literal and ends the form early.

### 3. Brackets inside comments break parsing

```
$ printf '(a ; comment with ) bracket\n b)\n' | sexpfmt
ERROR: Parse error ...
```

### 4. Top-level strings containing spaces break

```
$ echo '"hello world"' | sexpfmt
ERROR: Parse error at line 1, column 1 (offset 0): Unexpected input: '"hello'
```

Atom-forms split at whitespace, so the string is cut in half.

**Suggested fix:** make `FormReader` minimally token-aware — track an
in-string flag (with escape handling) and an in-comment flag in
`get_list_without_whitespace_prefix`, and treat `"` / `;` specially when a
form starts with them. That one change fixes all four bugs while keeping the
streaming design. The alternative (a single incremental parser replacing the
two-phase design) is cleaner long-term but a much bigger rewrite; the TODO at
`src/reader.rs:1` already gestures at this.

## Other confirmed bugs

### 5. `\\` escape is not understood

`src/parser.rs:122` — the only escape rule is `\"`, so a string ending in an
escaped backslash misparses (`\\"` is read as escaped-quote followed by an
unterminated string):

```
$ printf '(x "a\\\\")' | sexpfmt
ERROR: Parse error ...
```

Recognize `\\` (or generically `\` followed by any char).

### 6. NUL bytes inside strings are silently deleted

A side effect of the `[char; 2]` + `'\0'` sentinel hack in `string_atom`
(`src/parser.rs:95-113`):

```
$ printf '(a "x\0y")' | sexpfmt
(a "xy")
```

Return an enum or `String` per string element instead of a sentinel-padded
array; it also simplifies the code.

### 7. Printer stops wrapping at nesting depth 40

`available_width == 0` is the "unlimited" sentinel (`src/printer.rs:66`), but
multiline planning subtracts `INDENT_WIDTH` per level (`src/printer.rs:73`),
so at exactly `MARGIN_WIDTH / INDENT_WIDTH` = 40 levels the budget *hits the
sentinel* and huge inner lists are forced onto one line (reproduced with a
255-character output line). Use `Option<i32>` (or `i32::MAX`) to mean
"unlimited".

### 8. Panic on broken pipe

```
$ python3 -c "print('(a b c)\n' * 200000)" | sexpfmt | head -1
(a b c)
thread 'main' panicked at ...: failed printing to stdout: Broken pipe (os error 32)
```

Exit code 101 with a backtrace. `print!` / `println!` panic on EPIPE. The fix
falls out of the printer API change below (handle `ErrorKind::BrokenPipe` and
exit cleanly).

### 9. CLI arguments are silently ignored

`sexpfmt --help` reads stdin and exits 0; `sexpfmt file.sexp` hangs forever
waiting on stdin. At minimum, error out on unexpected args; ideally implement
what the README TODO already lists (`--help`, `--version`, margin/indent
flags, file inputs). `lexopt` / `pico-args` are near-zero-cost options;
`clap` if the dependency is acceptable.

## API and design improvements

- **Make the printer write to a `W: io::Write` instead of using `print!`**
  (`src/printer.rs:48-141`). Highest-leverage refactor: fixes the EPIPE
  panic, makes the printer unit-testable (it currently has zero Rust tests —
  only shell snapshots), makes the library usable as a library, and allows
  locking/buffering stdout once instead of taking the lock per token and
  flushing per form (the perf concern RELEASE.md 0.2.1 itself notes).
- **Empty `SExp::List` breaks the printer**: `es.len() - 1` underflows
  (`src/printer.rs:65`), `es_pps[0]` can index out of bounds
  (`src/printer.rs:99`), `.max().unwrap()` can panic (`src/printer.rs:75`).
  Unreachable via the parser (empty lists become `Null`), but `SExp` is
  public API — handle it or make the state unrepresentable.
- **`parse_form(text: String, ...)` should take `&str`** — it only calls
  `text.as_str()`.
- **Widths are byte counts** (`v.len()`, `src/printer.rs:60`), so multibyte
  UTF-8 atoms (CJK, emoji, accents) cause premature wrapping; `Loc` columns
  are byte-based too (acknowledged in the `src/reader.rs` TODO). The
  `unicode-width` crate is the standard fix.
- **`ByteReader` issues one `read()` call per byte** (`src/reader.rs:172`).
  Tolerable for locked stdin, but a syscall per byte for any file/socket a
  library user passes. Wrap the inner reader in `BufReader`.
- **Error positions point at the form start, not the offending byte** —
  `pop_bookend!` reports the captured start-of-form `position`
  (`src/reader.rs:64`) though `self.inner.peek_loc()` is available.
- Small ones: `basic_list` re-derives the bookend style from the char with a
  `panic!` arm when the `sexp_bookend_style` parameter is already in scope
  (`src/parser.rs:70-75`); the `Utf8` error variant is never constructed by
  the crate itself; the error tests living in `src/lib.rs` belong in
  `src/error.rs`.

## Testing gaps

The test corpus contains no comments and no strings with special characters —
precisely where all the confirmed bugs live.

- Add regression tests for bugs 1-9 (most become one-liners once the printer
  writes to a buffer).
- Add property tests: round-trip `parse(print(x)) == x` and idempotency
  `print(parse(print(x))) == print(x)`. `proptest` with a small `SExp`
  generator would have caught the depth-40 and NUL bugs; `cargo-fuzz` on the
  reader would have caught the rest.

## CI, scripts, dependencies, docs

- **CI clippy misses test code**: `cargo clippy -- -D warnings`
  (`.github/workflows/ci.yml:27`) doesn't lint tests; `--all-targets`
  reveals 8 existing `clone_on_copy` warnings. Also: no `cargo fmt --check`
  despite `rustfmt.toml`; `actions/checkout@v3` is deprecated (publish.yml
  already uses v4); `Swatinem/rust-cache` would speed up CI.
- **`script/test.sh:9` runs `bash "script/test_impl.sh"`** relative to the
  CWD, not `$ROOT` — it breaks when invoked from outside the repo root,
  defeating the `realpath` work above it (which also has an unquoted
  `$ROOT`).
- **`script/test_impl.sh`**: Python3 is a test dependency solely to print
  `"=" * 80` (the README dutifully tells users to install it) —
  `printf '=%.0s' {1..80}` drops the dependency. `expect_files_equal` shells
  out to `diff | wc -l` instead of using diff's exit code; `$NAME` is passed
  around but never set; temp files leak on failure; the test list is
  hardcoded where a glob over `test/*.sexp` (with the expected exit code
  encoded in the `-error_` filename convention) would auto-discover new
  tests.
- Consider gitignoring `test/.results/` — it is regenerated on every test
  run and produces diff churn (test ordering is nondeterministic).
- **Dependencies**: nom 7 → nom 8 (+ nom_locate 5), thiserror 1 → 2. Not
  urgent, but worth doing while the parser is being touched.
- **RELEASE.md stops at 0.2.2 while Cargo.toml says 1.0.1** — backfill or
  have the publish workflow maintain it.

## Suggested priority

1. Fix the reader/parser grammar mismatch (bugs 1-4) — one silent-corruption
   bug lives here.
2. Printer-to-`io::Write` refactor — fixes the EPIPE panic and unlocks unit
   testing in one move.
3. String escapes (`\\`) and the NUL sentinel (bugs 5-6).
4. Argument handling (`--help` at minimum, error on unknown args).
5. Depth-40 sentinel (bug 7), then the hygiene items (CI flags, scripts,
   deps, docs) opportunistically.
