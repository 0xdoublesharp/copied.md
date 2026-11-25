#!/bin/sh
set -eu

source_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
jar=${TLA2TOOLS_JAR:-"$HOME/.local/share/tla2tools/tla2tools.jar"}
if [ ! -f "$jar" ]; then
  printf 'Set TLA2TOOLS_JAR to a readable tla2tools.jar (tried %s).\n' "$jar" >&2
  exit 1
fi
jar_dir=$(CDPATH='' cd -- "$(dirname -- "$jar")" && pwd)
jar="$jar_dir/$(basename -- "$jar")"

if command -v java >/dev/null 2>&1 && java -version >/dev/null 2>&1; then
  java_bin=java
elif [ -x /opt/homebrew/opt/openjdk/bin/java ]; then
  java_bin=/opt/homebrew/opt/openjdk/bin/java
else
  printf '%s\n' 'Install OpenJDK or put a working java on PATH.' >&2
  exit 1
fi

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
cp "$source_dir/"*.tla "$source_dir/"*.cfg "$run_dir/"
cd "$run_dir"

run_tlc() {
  "$java_bin" -Xmx512m -Djava.io.tmpdir="$run_dir" -cp "$jar" tlc2.TLC \
    -nowarning -workers "${COMPUTE_JOBS:-1}" -metadir "$run_dir/$2" -config "$2.cfg" "$1"
}

expect_counterexample() {
  config=$1
  invariant=$2
  exit_code=0
  run_tlc Clipboard "$config" > "$config.log" 2>&1 || exit_code=$?
  if [ "$exit_code" -ne 12 ] ||
    ! grep -Fq "Invariant $invariant is violated." "$config.log"; then
    cat "$config.log"
    exit 1
  fi
  printf 'TLC reproduced the expected %s counterexample.\n' "$invariant"
}

run_tlc Clipboard Clipboard
expect_counterexample LegacyClipboard NeverWriteAfterAnObservedChange
expect_counterexample ExternalRace NoExternalRace
run_tlc WinBoard WinBoard
run_tlc CopyAndPick CopyAndPick
run_tlc Engine Engine
