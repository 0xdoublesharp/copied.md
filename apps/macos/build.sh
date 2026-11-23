#!/bin/sh
set -eu

root_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH= cd -- "$root_dir/../.." && pwd)
output_dir=${COPIED_MD_MACOS_OUT:-$repo_dir/build/macos}
mkdir -p "$output_dir/module-cache"
TMPDIR="$output_dir" xcrun swiftc -parse-as-library -module-cache-path "$output_dir/module-cache" \
  "$root_dir/Pasteboard.swift" "$root_dir/PasteboardCommand.swift" -o "$output_dir/copied-md-pasteboard"
printf '%s\n' "$output_dir/copied-md-pasteboard"
