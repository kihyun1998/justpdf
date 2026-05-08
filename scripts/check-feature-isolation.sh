#!/usr/bin/env bash
# Verifies that crate feature flags isolate optional dependencies as documented
# in ADR-0001 (external dependency layering).
#
# Each assertion below corresponds to one acceptance criterion of issue #1.

set -uo pipefail

cd "$(dirname "$0")/.."

fail=0

assert_dep_absent() {
  local pkg=$1 dep=$2 features=$3 label=$4
  local out
  if [ -n "$features" ]; then
    out=$(cargo tree -p "$pkg" --depth 1 --edges no-dev --no-default-features --features "$features" 2>/dev/null) || { echo "FAIL [$label]: cargo tree failed" >&2; fail=1; return; }
  else
    out=$(cargo tree -p "$pkg" --depth 1 --edges no-dev 2>/dev/null) || { echo "FAIL [$label]: cargo tree failed" >&2; fail=1; return; }
  fi
  if printf '%s\n' "$out" | grep -q "^[├└]── ${dep} "; then
    echo "FAIL [$label]: $pkg has unexpected dependency on $dep" >&2
    fail=1
  else
    echo "PASS [$label]: $dep absent from $pkg"
  fi
}

assert_dep_present() {
  local pkg=$1 dep=$2 features=$3 label=$4
  local out
  if [ -n "$features" ]; then
    out=$(cargo tree -p "$pkg" --depth 1 --edges no-dev --no-default-features --features "$features" 2>/dev/null) || { echo "FAIL [$label]: cargo tree failed" >&2; fail=1; return; }
  else
    out=$(cargo tree -p "$pkg" --depth 1 --edges no-dev 2>/dev/null) || { echo "FAIL [$label]: cargo tree failed" >&2; fail=1; return; }
  fi
  if printf '%s\n' "$out" | grep -q "^[├└]── ${dep} "; then
    echo "PASS [$label]: $dep present in $pkg"
  else
    echo "FAIL [$label]: $pkg should depend on $dep but does not" >&2
    fail=1
  fi
}

assert_build_ok() {
  local pkg=$1 features=$2 label=$3
  if [ -n "$features" ]; then
    if cargo build -p "$pkg" --quiet --no-default-features --features "$features" 2>/dev/null; then
      echo "PASS [$label]: $pkg builds with features='$features'"
    else
      echo "FAIL [$label]: $pkg fails to build with features='$features'" >&2
      fail=1
    fi
  else
    if cargo build -p "$pkg" --quiet 2>/dev/null; then
      echo "PASS [$label]: $pkg builds (default features)"
    else
      echo "FAIL [$label]: $pkg fails to build (default features)" >&2
      fail=1
    fi
  fi
}

# --- justpdf-special (issue #1) ---
# Default features must NOT pull justpdf-render
assert_dep_absent justpdf-special justpdf-render "" "special/default"
# ocr feature must pull justpdf-render
assert_dep_present justpdf-special justpdf-render "ocr" "special/ocr"
# Non-ocr feature subset must build without render stack
assert_build_ok justpdf-special "barcode" "special/barcode-only"
# All features must still build
assert_build_ok justpdf-special "all" "special/all"

# --- justpdf-formats (issue #2) ---
# Default features must NOT pull justpdf-render
assert_dep_absent justpdf-formats justpdf-render "" "formats/default"
# Conversion-only feature subsets must NOT pull justpdf-render
assert_dep_absent justpdf-formats justpdf-render "xps" "formats/xps"
assert_dep_absent justpdf-formats justpdf-render "epub" "formats/epub"
assert_dep_absent justpdf-formats justpdf-render "svg" "formats/svg"
assert_dep_absent justpdf-formats justpdf-render "office" "formats/office"
assert_dep_absent justpdf-formats justpdf-render "cbz" "formats/cbz"
# Preview-generating features MUST pull justpdf-render
assert_dep_present justpdf-formats justpdf-render "fb2" "formats/fb2"
assert_dep_present justpdf-formats justpdf-render "mobi" "formats/mobi"
assert_dep_present justpdf-formats justpdf-render "plaintext" "formats/plaintext"
# All features must still build
assert_build_ok justpdf-formats "all" "formats/all"

exit "$fail"
