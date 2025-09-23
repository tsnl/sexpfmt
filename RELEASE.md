# Release Notes

## 0.2.2

- Better error messages including the line number, column number, and byte offset of the first
  offending byte.

## 0.2.1

- Unbuffer output: flush when each datum is printed out. This will most likely result in performance degradation. However, it fixes subtle and confusing bugs where sexpfmt will consume input but no output will be produced (the output is buffered but not yet flushed).

## 0.2.0

- Streaming: forms are now formatted and emitted one-at-a-time.
  - This is a breaking change: old behavior was like `sponge` on the whole file.
    Now, we only `sponge` one top-level form at a time.
  - Future releases may `sponge` even less: this behavior is unspecified going forward.
- More explicit I/O error handling.

## 0.1.0

- Basic CLI tool: read input as UTF-8 from `stdin` until EOF, format, then print to `stdout`.
