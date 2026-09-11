#!/usr/bin/env bash
# `query` alone must pull no crypto provider: the application chooses one.
#
# `cargo tree -i` exits 0 with "nothing to print" for a package that is in the
# lockfile but unreachable, so reachability is read from the output rather than
# the exit status. `-e normal` excludes dev-dependencies, which no consumer
# resolves.
set -euo pipefail

fail=0

reachable() { # reachable PKG FEATURES
  cargo tree -e normal -i "$1" --no-default-features --features "$2" 2>/dev/null \
    | head -1 | grep -q "^$1 v"
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
