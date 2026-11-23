#!/bin/sh
set -eu

if [ ! -d /Volumes/Scratch ]; then
  printf '%s\n' 'Mount /Volumes/Scratch before building the pasteboard helper.' >&2
  exit 1
fi

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
output_dir=${COPIED_MD_MACOS_OUT:-/Volumes/Scratch/copied-md/build/macos}
mkdir -p "$output_dir/module-cache"
TMPDIR="$output_dir" xcrun swiftc -parse-as-library -module-cache-path "$output_dir/module-cache" \
  "$root_dir/Pasteboard.swift" "$root_dir/PasteboardCommand.swift" -o "$output_dir/copied-md-pasteboard"
printf '%s\n' "$output_dir/copied-md-pasteboard"
