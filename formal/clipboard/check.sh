#!/bin/sh
set -eu

source_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
: "${TLA2TOOLS_JAR:?Set TLA2TOOLS_JAR to the path of the tla2tools.jar release}"
jar_dir=$(CDPATH='' cd -- "$(dirname -- "$TLA2TOOLS_JAR")" && pwd)
jar="$jar_dir/$(basename -- "$TLA2TOOLS_JAR")"
test -f "$jar"

if [ -n "${COPIED_MD_FORMAL_OUT:-}" ]; then
  output=$COPIED_MD_FORMAL_OUT
elif [ "$(uname -s)" = Darwin ] && [ -d /Volumes/Scratch ] &&
  [ "$(stat -f %d /Volumes/Scratch)" != "$(stat -f %d /Volumes)" ]; then
  output=/Volumes/Scratch/copied-md/tmp/formal
else
  printf '%s\n' 'Set COPIED_MD_FORMAL_OUT to a directory for disposable model checker output.' >&2
  exit 1
fi
mkdir -p "$output"
output=$(CDPATH='' cd -- "$output" && pwd)
run_dir=$(mktemp -d "$output/clipboard.XXXXXX")
trap 'rm -rf -- "$run_dir"' EXIT
cp "$source_dir/Clipboard.tla" "$source_dir/"*.cfg "$run_dir/"
cd "$run_dir"

run_tlc() {
  config=$1
  java -Xmx512m -Djava.io.tmpdir="$run_dir" -cp "$jar" tlc2.TLC \
    -nowarning -workers 1 -metadir "$run_dir/$config" -config "$config.cfg" Clipboard
}

expect_counterexample() {
  config=$1
  invariant=$2
  exit_code=0
  run_tlc "$config" > "$config.log" 2>&1 || exit_code=$?
  if [ "$exit_code" -ne 12 ] ||
    ! grep -Fq "Invariant $invariant is violated." "$config.log"; then
    cat "$config.log"
    exit 1
  fi
  printf 'TLC reproduced the expected %s counterexample.\n' "$invariant"
}

run_tlc Clipboard
expect_counterexample LegacyClipboard NeverWriteAfterAnObservedChange
expect_counterexample ExternalRace NoExternalRace
