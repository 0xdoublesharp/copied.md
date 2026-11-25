#!/bin/sh
set -eu

source_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH= cd -- "$source_dir/../.." && pwd)
output=${COPIED_MD_APP_OUT:-$repo_dir/build/portable}
runtime=${COPIED_MD_NODE_CACHE:-$repo_dir/build/macos-runtimes}
version=24.22.1
identity=${COPIED_MD_SIGN_IDENTITY:--}
notary_profile=${COPIED_MD_NOTARY_PROFILE:-}

if [ -n "$notary_profile" ] && [ "$identity" = "-" ]; then
  printf '%s\n' 'Notarization requires a Developer ID Application identity in COPIED_MD_SIGN_IDENTITY.' >&2
  exit 1
fi

mkdir -p "$output" "$runtime"
output=$(CDPATH= cd -- "$output" && pwd)
runtime=$(CDPATH= cd -- "$runtime" && pwd)
if [ -e "$output/copied.md.app" ] || [ -e "$output/copied-md-macos-universal.zip" ]; then
  printf '%s\n' 'The output already contains a copied.md package. Choose a new COPIED_MD_APP_OUT directory.' >&2
  exit 1
fi

stage=$(mktemp -d "$output/.copied-md-build.XXXXXX")
trap 'rm -rf "$stage"' EXIT
export TMPDIR="$stage"
app="$stage/copied.md.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Helpers" "$app/Contents/Resources" "$stage/module-cache"
cp "$source_dir/Info.plist" "$app/Contents/Info.plist"

(
  cd "$runtime"
  curl_node() {
    curl -fsSL --proto '=https' --retry 1 --max-time 90 "https://nodejs.org/dist/v$version/$1" -o "$2"
  }
  manifest="SHASUMS256-v$version.txt"
  if [ ! -f "$manifest" ]; then
    curl_node SHASUMS256.txt "$stage/$manifest"
    mv "$stage/$manifest" "$manifest"
  fi
  for cpu in arm64 x64; do
    archive="node-v$version-darwin-$cpu.tar.gz"
    if [ ! -f "$archive" ]; then
      curl_node "$archive" "$stage/$archive"
      mv "$stage/$archive" "$archive"
    fi
    awk -v archive="$archive" '$2 == archive { print }' "$manifest" | shasum -a 256 -c -
    tar -xzf "$archive" -C "$stage" "node-v$version-darwin-$cpu/bin/node" "node-v$version-darwin-$cpu/LICENSE"
  done
)

xcrun lipo -create \
  "$stage/node-v$version-darwin-arm64/bin/node" "$stage/node-v$version-darwin-x64/bin/node" \
  -output "$app/Contents/Helpers/node"
cp "$stage/node-v$version-darwin-arm64/LICENSE" "$app/Contents/Resources/Node-LICENSE.txt"

COPIED_MD_CLI_OUT="$app/Contents/Resources/converter.mjs" \
  pnpm --dir "$repo_dir" --filter @0xdoublesharp/copied-md-cli build

for cpu in arm64 x86_64; do
  xcrun swiftc -parse-as-library -O -target "$cpu-apple-macos13.5" \
    -module-cache-path "$stage/module-cache" \
    "$source_dir/Pasteboard.swift" "$source_dir/GlobalHotKey.swift" \
    "$source_dir/FormatPicker.swift" "$source_dir/FormatIcons.swift" \
    "$source_dir/ConverterRuntime.swift" "$source_dir/DesktopApp.swift" \
    -o "$stage/copied-md-$cpu"
  xcrun swiftc -parse-as-library -O -target "$cpu-apple-macos13.5" \
    -module-cache-path "$stage/module-cache" \
    "$source_dir/Pasteboard.swift" "$source_dir/FormatPicker.swift" \
    "$source_dir/FormatIcons.swift" "$source_dir/PasteboardCommand.swift" \
    -o "$stage/copied-md-pasteboard-$cpu"
done
xcrun lipo -create "$stage/copied-md-arm64" "$stage/copied-md-x86_64" -output "$app/Contents/MacOS/copied-md"
xcrun lipo -create "$stage/copied-md-pasteboard-arm64" "$stage/copied-md-pasteboard-x86_64" \
  -output "$app/Contents/Helpers/copied-md-pasteboard"

mkdir "$stage/AppIcon.iconset"
icon="$repo_dir/assets/copied-md.png"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$icon" --out "$stage/AppIcon.iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$icon" --out "$stage/AppIcon.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$stage/AppIcon.iconset" -o "$app/Contents/Resources/AppIcon.icns"

if [ "$identity" = "-" ]; then
  codesign --force --sign - "$app/Contents/Helpers/node"
  codesign --force --sign - "$app/Contents/Helpers/copied-md-pasteboard"
  codesign --force --sign - "$app"
else
  codesign --force --sign "$identity" --options runtime --timestamp \
    --entitlements "$source_dir/Node.entitlements" "$app/Contents/Helpers/node"
  codesign --force --sign "$identity" --options runtime --timestamp "$app/Contents/Helpers/copied-md-pasteboard"
  codesign --force --sign "$identity" --options runtime --timestamp "$app"
fi
plutil -lint "$app/Contents/Info.plist"
for executable in "$app/Contents/MacOS/copied-md" "$app/Contents/Helpers/"*; do
  xcrun lipo "$executable" -verify_arch arm64 x86_64
done
codesign --verify --deep --strict "$app"

if [ -n "$notary_profile" ]; then
  if ! codesign --display --verbose=2 "$app" 2>&1 | grep -q '^Authority=Developer ID Application:'; then
    printf '%s\n' 'The app must be signed with a Developer ID Application certificate to notarize it.' >&2
    exit 1
  fi
  submission="$stage/notary-submission.zip"
  result="$stage/notary-result.json"
  ditto -c -k --keepParent "$app" "$submission"
  if ! xcrun notarytool submit "$submission" --keychain-profile "$notary_profile" --wait \
    --output-format json > "$result" || [ "$(plutil -extract status raw -o - "$result")" != Accepted ]; then
    cat "$result" >&2
    printf '%s\n' 'Apple did not accept the notarization submission.' >&2
    exit 1
  fi
  xcrun stapler staple "$app"
  xcrun stapler validate "$app"
  codesign --verify --deep --strict "$app"
  if command -v syspolicy_check >/dev/null 2>&1; then
    syspolicy_check distribution "$app"
  else
    spctl --assess --type execute "$app"
  fi
fi

ditto -c -k --keepParent "$app" "$stage/copied-md-macos-universal.zip"
mv "$app" "$stage/copied-md-macos-universal.zip" "$output/"
printf '%s\n' "$output/copied.md.app" "$output/copied-md-macos-universal.zip"
