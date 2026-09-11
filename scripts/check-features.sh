#!/usr/bin/env bash
# `query` alone must pull no crypto provider: the application chooses one.
#
# `cargo tree -i` exits 0 with "nothing to print" for a package that is in the
# lockfile but unreachable, so reachability is read from the output rather than
# the exit status. `-e normal` excludes dev-dependencies, which no consumer
# resolves.
set -euo pipefail

stderr_file=$(mktemp)
trap 'rm -f "$stderr_file"' EXIT

fail=0

# reachable PKG FEATURES — true when PKG is in the normal-dependency graph.
#
# A broken cargo invocation is not an absence. Reporting one as "not reachable"
# would turn a check that never ran into a passing invariant, so anything but a
# clean run or a definite "no such package" aborts the script.
reachable() {
  local pkg=$1 features=$2 out status first
  status=0
  out=$(cargo tree -e normal -i "$pkg" --no-default-features --features "$features" 2>"$stderr_file") || status=$?

  if [ "$status" -ne 0 ]; then
    if grep -q 'did not match any packages' "$stderr_file"; then
      return 1
    fi
    echo "ERROR: cargo tree failed for '$pkg' with --features $features (exit $status)" >&2
    cat "$stderr_file" >&2
    exit 2
  fi

  # Read the first line without a pipe: under `pipefail`, `head -1` closing the
  # pipe early can surface as a failure and read as an absence.
  first=${out%%$'\n'*}
  [[ $first == "$pkg v"* ]]
}

if reachable aws-lc-sys query; then
  echo "FAIL: aws-lc-sys is reachable from --features query"
  fail=1
else
  echo "ok: query pulls no aws-lc-sys"
fi

if reachable ring query; then
  echo "FAIL: ring is reachable from --features query"
  fail=1
else
  echo "ok: query pulls no ring"
fi

if reachable aws-lc-sys query,tls; then
  echo "ok: query,tls pulls aws-lc-sys"
else
  echo "FAIL: query,tls does not pull aws-lc-sys"
  fail=1
fi

if reachable ring query,tls-ring; then
  echo "ok: query,tls-ring pulls ring"
else
  echo "FAIL: query,tls-ring does not pull ring"
  fail=1
fi

exit "$fail"
