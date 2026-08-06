/* C API for sexpfmt: format S-expressions in a consistent style that is both
 * line-diffable and human-readable.
 *
 * Build the library from the repository root with:
 *
 *   cargo rustc --release --features capi --lib --crate-type cdylib    # shared
 *   cargo rustc --release --features capi --lib --crate-type staticlib # static
 *
 * The artifacts land in target/release/ (libsexpfmt.so / .dylib / .dll and
 * libsexpfmt.a respectively).
 *
 * All strings returned by this API are allocated with malloc and must be
 * released with sexpfmt_str_free().
 */
#ifndef SEXPFMT_H
#define SEXPFMT_H

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Return codes for sexpfmt_format(). */
enum {
	SEXPFMT_OK = 0,                   /* success */
	SEXPFMT_ERR_FORMAT = 1,           /* input could not be parsed/formatted */
	SEXPFMT_ERR_INVALID_ARGUMENT = 2, /* null input/out, or bad config value */
	SEXPFMT_ERR_NOMEM = 3             /* allocating the result failed */
};

/* Values for sexpfmt_config.bookends. */
enum {
	SEXPFMT_BOOKENDS_KEEP = -1,  /* preserve each list's input style */
	SEXPFMT_BOOKENDS_PARENS = 0, /* normalize to ( ) */
	SEXPFMT_BOOKENDS_SQUARE = 1, /* normalize to [ ] */
	SEXPFMT_BOOKENDS_CURLY = 2   /* normalize to { } */
};

/* Formatting options; mirrors the sexpfmt CLI options one-to-one. */
typedef struct sexpfmt_config {
	/* Number of spaces per indentation level (--indent; default 2). */
	size_t indent_width;
	/* Target maximum line width (--margin; default 80). */
	size_t margin_width;
	/* One of SEXPFMT_BOOKENDS_* (--bookends; default KEEP). */
	int bookends;
	/* Preserve `;` line comments instead of discarding them
	 * (--preserve-comments; default false). */
	bool preserve_comments;
	/* In multi-line lists, keep a `:label` atom on the same line as the
	 * element that follows it (--pair-labels; default false). */
	bool pair_labels;
} sexpfmt_config;

/* Returns the default configuration (the same defaults as the CLI). */
sexpfmt_config sexpfmt_config_default(void);

/* Formats input_len bytes of S-expression source from input.
 *
 * On success, returns SEXPFMT_OK and stores a NUL-terminated, malloc'ed UTF-8
 * buffer in *out. The formatted text can contain interior NUL bytes (a string
 * literal may contain a literal NUL), so the exact byte length is also stored
 * in *out_len when out_len is non-null.
 *
 * On failure, returns a nonzero error code, *out is NULL, and when error is
 * non-null, *error holds a NUL-terminated, malloc'ed message.
 *
 * config may be NULL, in which case the defaults apply. input need not be
 * NUL-terminated, and may be NULL only when input_len is 0. out must be
 * non-null; out_len and error may each be NULL. Release *out and *error with
 * sexpfmt_str_free().
 */
int sexpfmt_format(const char *input, size_t input_len,
                   const sexpfmt_config *config,
                   char **out, size_t *out_len, char **error);

/* Releases a string returned by this API. Passing NULL is a no-op. */
void sexpfmt_str_free(char *s);

#ifdef __cplusplus
}
#endif

#endif /* SEXPFMT_H */
