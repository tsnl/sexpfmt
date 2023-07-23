ROOT="$(dirname "$0")/../"
VERBOSE=0

TESTS_FAIL_COUNT=0
TESTS_GENERATED_COUNT=0

DIVIDER0=$(python3 -c 'print("=" * 80)')
DIVIDER1=$(python3 -c 'print("-" * 80)')
DIVIDER2=$(python3 -c 'print("." * 80)')

SEXPFMT="$ROOT/target/release/sexpfmt"

function print_usage () {
    echo "USAGE:"
    echo "- $0 clean        # cleans saved expected output"
    echo "- $0 verbose      # runs tests, saving output if unavailable, else diffing; also prints output on failure"
    echo "- $0              # runs tests, saving output if unavailable, else diffing"
    exit 1
}

function setup () {
    local SOUT=$(mktemp)
    local SERR=$(mktemp)

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
    local SOUT=$(mktemp)
    local SERR=$(mktemp)

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
    local NAME="$1"
    local ACTUAL="$2"
    local EXPECT="$3"

    if [[ -f "$EXPECT" ]]; then 
        # Output already recorded
        if [ $(diff "$ACTUAL" "$EXPECT" | wc -l) -ne 0 ]; then
            return 1
        fi
    else
        # No output available, hence saving.
        cp $ACTUAL $EXPECT
        TESTS_GENERATED_COUNT=$(expr $TESTS_GENERATED_COUNT + 1)
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

    echo "$(dirname -- $FILE)/.expect/$(basename -- $FILE).$SUFFIX"
}

function expect_test_output () {
    local NAME="$1"
    local FILE="$2"
    local TOUT="$3"
    local TERR="$4"

    expect_files_equal "$NAME" "$TOUT" "$(get_expected_file $FILE 'out')"
    TOUT_OK=$?
    if [ $TOUT_OK -ne 0 ]; then
        return 1
    fi

    expect_files_equal "$NAME" "$TERR" "$(get_expected_file $FILE 'err')"
    TERR_OK=$?
    if [ $TERR_OK -ne 0 ]; then
        return 1
    fi
    return 0
}

function test_file () {
    local EXPECT_EC=$1
    local FILE=$2
    local TOUT=$(mktemp)
    local TERR=$(mktemp)

    local CWD=$(pwd)
    local SHORT_FILE=$(basename "$FILE")

    echo -n "TEST: '$SHORT_FILE' ... "

    cat "$FILE" | "$SEXPFMT" 1> "$TOUT" 2> "$TERR"
    local TEST_EC="${PIPESTATUS[1]}"
    
    if [ $TEST_EC -ne $EXPECT_EC ]; then
        echo "FAIL: invalid EC: expected $EXPECT_EC, got $TEST_EC"
        if [ $VERBOSE -ne 0 ]; then
            display_output "$TOUT" "$TERR"
        fi
        TESTS_FAIL_COUNT=$(expr $TESTS_FAIL_COUNT + 1)
        return $TEST_EC
    fi

    expect_test_output "$NAME" "$FILE" "$TOUT" "$TERR"
    local EXPECT_TEST_EC=$?
    if [ $EXPECT_TEST_EC -ne 0 ]; then
        echo "FAIL: expect tests failed."
        if [ $VERBOSE -ne 0 ]; then
            display_output "$TOUT" "$TERR"
        fi
        TESTS_FAIL_COUNT=$(expr $TESTS_FAIL_COUNT + 1)
        return $TEST_EC
    fi

    echo "PASS"

    rm -f $TOUT
    rm -f $TERR
    
    return 0
}

#
# Main application:
#

if [ $# -eq 1 ]; then
    if [ "$1" = "clean" ]; then
        clean_expects
        exit $?
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

echo $DIVIDER0
echo -n "SETUP... "
setup
if [ "$?" -ne 0 ]; then
  exit 1
fi

echo $DIVIDER0
echo -n "UNIT TESTS... "
unittest
if [ "$?" -ne 0 ]; then
  exit 1
fi

echo $DIVIDER0
echo "EXPECT TESTS"
mkdir -p "$ROOT/test/.expect"
test_file 0 "$ROOT/test/test001-cafe_order_1.sexp"
test_file 0 "$ROOT/test/test002-multiline_head.sexp"
test_file 0 "$ROOT/test/test003-various_bookends.sexp"
test_file 0 "$ROOT/test/test004-ast1.sexp"

if [ "$TESTS_GENERATED_COUNT" -ne 0 ]; then
    echo "INFO: $TESTS_GENERATED_COUNT outputs generated."
fi

echo $DIVIDER0
if [ "$TESTS_FAIL_COUNT" -ne 0 ]; then
    echo "FAILURE: $TESTS_FAIL_COUNT tests failed"
    exit 1
else 
    echo "SUCCESS: OK"
    exit 0
fi
