ROOT="$(dirname "$0")/.."
ROOT="$(realpath "$ROOT")"
VERBOSE=0

TESTS_FAIL_COUNT=0
TESTS_GENERATED_COUNT=0

DIVIDER0=$(printf '=%.0s' {1..80})
DIVIDER1=$(printf -- '-%.0s' {1..80})
DIVIDER2=$(printf '.%.0s' {1..80})

SEXPFMT="$ROOT/target/release/sexpfmt"

# All temporary files live in one directory that is removed on exit.
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

function print_usage () {
    echo "USAGE:"
    echo "- $0 clean        # cleans saved expected output"
    echo "- $0 verbose      # runs tests, saving output if unavailable, else diffing; also prints output on failure"
    echo "- $0              # runs tests, saving output if unavailable, else diffing"
    exit 1
}

function setup () {
    local SOUT="$TMP_DIR/setup.out"
    local SERR="$TMP_DIR/setup.err"

    cargo build --release 1> "$SOUT" 2> "$SERR"
    local SETUP_EC=$?

    if [ $SETUP_EC -ne 0 ]; then
        echo "FAILED"
        display_output "$SOUT" "$SERR"
    else
        echo "OK"
    fi

    return $SETUP_EC
}

function unittest () {
    local SOUT="$TMP_DIR/unittest.out"
    local SERR="$TMP_DIR/unittest.err"

    cargo test --release 1> "$SOUT" 2> "$SERR"
    local SETUP_EC=$?

    if [ $SETUP_EC -ne 0 ]; then
        echo "FAILED"
        display_output "$SOUT" "$SERR"
    else
        echo "OK"
        cat "$SOUT"
    fi

    return $SETUP_EC
}

function display_output () {
    local TOUT="$1"
    local TERR="$2"

    echo "$DIVIDER1"
    echo "... STDOUT:"
    cat "$TOUT"
    echo
    echo "$DIVIDER2"
    echo "... STDERR:"
    cat "$TERR"
    echo
    echo "$DIVIDER1"
}

function expect_files_equal () {
    local ACTUAL="$1"
    local EXPECT="$2"

    if [[ -f "$EXPECT" ]]; then
        # Output already recorded
        if ! diff -q "$ACTUAL" "$EXPECT" > /dev/null; then
            return 1
        fi
    else
        # No output available, hence saving.
        cp "$ACTUAL" "$EXPECT"
        TESTS_GENERATED_COUNT=$(("$TESTS_GENERATED_COUNT" + 1))
    fi

    return 0
}

function clean_expects () {
    echo "INFO: Cleaning..."
    rm "$ROOT"/test/.expect/*
    local EC=$?
    if [ $EC -ne 0 ]; then
        echo "ERROR: Cleaning failed"
    fi
    exit $EC
}

function get_expected_file () {
    local FILE="$1"
    local SUFFIX="$2"

    echo "$(dirname -- "$FILE")/.expect/$(basename -- "$FILE").$SUFFIX"
}

function expect_test_output () {
    local FILE="$1"
    local TOUT="$2"
    local TERR="$3"

    if ! expect_files_equal "$TOUT" "$(get_expected_file "$FILE" 'out')"; then
        return 1
    fi
    if ! expect_files_equal "$TERR" "$(get_expected_file "$FILE" 'err')"; then
        return 1
    fi
    return 0
}

function test_file () {
    local FILE="$1"
    local SHORT_FILE
    local EXPECT_EC
    local TEST_EC

    SHORT_FILE=$(basename "$FILE")
    local TOUT="$TMP_DIR/$SHORT_FILE.out"
    local TERR="$TMP_DIR/$SHORT_FILE.err"

    # Tests named `*-error_*` are expected to fail with exit code 1.
    case "$SHORT_FILE" in
        *-error_*) EXPECT_EC=1 ;;
        *) EXPECT_EC=0 ;;
    esac

    echo -n "TEST: '$SHORT_FILE' ... "

    "$SEXPFMT" < "$FILE" 1> "$TOUT" 2> "$TERR"
    TEST_EC="$?"

    if [ "$TEST_EC" -ne "$EXPECT_EC" ]; then
        echo "FAIL: invalid EC: expected $EXPECT_EC, got $TEST_EC"
        if [ $VERBOSE -ne 0 ]; then
            display_output "$TOUT" "$TERR"
        fi
        TESTS_FAIL_COUNT=$(("$TESTS_FAIL_COUNT" + 1))
        return 1
    fi

    if ! expect_test_output "$FILE" "$TOUT" "$TERR"; then
        echo "FAIL: output does not match expected output."
        if [ $VERBOSE -ne 0 ]; then
            display_output "$TOUT" "$TERR"
        fi
        TESTS_FAIL_COUNT=$(("$TESTS_FAIL_COUNT" + 1))
        return 1
    fi

    echo "PASS"
    return 0
}

#
# Main application:
#

if [ $# -eq 1 ]; then
    if [ "$1" = "clean" ]; then
        clean_expects
    elif [ "$1" = "verbose" ]; then
        VERBOSE=1
    else
        echo "ERROR: Invalid arguments."
        print_usage
    fi
elif [ $# -ne 0 ]; then
    echo "ERROR: Invalid arguments."
    print_usage
fi

echo "$DIVIDER0"
echo -n "SETUP... "
if ! setup; then
  exit 1
fi

echo "$DIVIDER0"
echo -n "UNIT TESTS... "
if ! unittest; then
  exit 1
fi

echo "$DIVIDER0"
echo "EXPECT TESTS"
mkdir -p "$ROOT/test/.expect"
for FILE in "$ROOT"/test/*.sexp; do
    test_file "$FILE"
done

if [ "$TESTS_GENERATED_COUNT" -ne 0 ]; then
    echo "INFO: $TESTS_GENERATED_COUNT outputs generated."
fi

echo "$DIVIDER0"
if [ "$TESTS_FAIL_COUNT" -ne 0 ]; then
    echo "FAILURE: $TESTS_FAIL_COUNT tests failed"
    exit 1
else
    echo "SUCCESS: OK"
    exit 0
fi
