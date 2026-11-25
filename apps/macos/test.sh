#!/bin/sh
set -eu

if [ "$(uname -s)" != Darwin ]; then
  printf '%s\n' 'Native tests require macOS.' >&2
  exit 1
fi

root_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_dir=$(CDPATH='' cd -- "$root_dir/../.." && pwd)
if [ -n "${COPIED_MD_MACOS_TEST_OUT:-}" ]; then
  output_dir=$COPIED_MD_MACOS_TEST_OUT
elif [ "${GITHUB_ACTIONS:-}" = true ]; then
  output_dir=$RUNNER_TEMP/copied-md-native
elif [ -d /Volumes/Scratch ] &&
  [ "$(stat -f %d /Volumes/Scratch)" != "$(stat -f %d /Volumes)" ]; then
  output_dir=/Volumes/Scratch/copied-md/build/native-test
else
  printf '%s\n' 'Mount /Volumes/Scratch or set COPIED_MD_MACOS_TEST_OUT to a build directory.' >&2
  exit 1
fi
mkdir -p "$output_dir"
run_dir=$(mktemp -d "$output_dir/run.XXXXXX")
trap 'rm -rf -- "$run_dir"' EXIT
export TMPDIR="$run_dir"
export COPIED_MD_CLI_OUT="$run_dir/converter.mjs"
cd "$repo_dir"
pnpm build:cli

compile() {
  xcrun swiftc -parse-as-library -target "$(uname -m)-apple-macos13.5" -profile-generate -profile-coverage-mapping \
    -module-cache-path "$run_dir/module-cache" "$@"
}

compile -D COPIED_MD_TESTS \
  "$root_dir/Pasteboard.swift" "$root_dir/ConverterRuntime.swift" "$root_dir/GlobalHotKey.swift" \
  "$root_dir/FormatPicker.swift" "$root_dir/FormatIcons.swift" \
  "$root_dir/DesktopApp.swift" "$root_dir/PasteboardCommand.swift" \
  "$root_dir/tests/NativeTests.swift" -o "$run_dir/native-tests"
LLVM_PROFILE_FILE="$run_dir/native.profraw" \
  "$run_dir/native-tests" "$run_dir/copied.md.app" "$(command -v node)" "$COPIED_MD_CLI_OUT" > "$run_dir/native.log"
cat "$run_dir/native.log"
if ! grep -q '^Native clipboard, picker, desktop, hotkey and converter runtime tests passed\.$' "$run_dir/native.log"; then
  printf '%s\n' 'The native test process exited before finishing the suite.' >&2
  exit 1
fi
LLVM_PROFILE_FILE="$run_dir/launch.profraw" \
  "$run_dir/native-tests" launch -hasLaunchedDesktopApp YES -keyboardShortcutDisabled YES

compile "$root_dir/Pasteboard.swift" "$root_dir/FormatPicker.swift" "$root_dir/FormatIcons.swift" \
  "$root_dir/PasteboardCommand.swift" -o "$run_dir/pasteboard-helper"
export LLVM_PROFILE_FILE="$run_dir/helper-%p.profraw"
helper=$run_dir/pasteboard-helper
"$helper" self-test

check_error() {
  command=$1
  expected_status=$2
  expected_message=$3
  input=$4
  shift 4
  status=0
  printf '%s' "$input" | "$command" "$@" >"$run_dir/stdout" 2>"$run_dir/stderr" || status=$?
  if [ "$status" -ne "$expected_status" ] || [ -s "$run_dir/stdout" ] ||
    ! grep -Fq "$expected_message" "$run_dir/stderr"; then
    printf 'Expected %s to exit %s with "%s". Got exit %s.\n' "$command" "$expected_status" "$expected_message" "$status" >&2
    cat "$run_dir/stdout" "$run_dir/stderr" >&2
    exit 1
  fi
}

check_error "$helper" 2 'Usage: copied-md-pasteboard' '' invalid
check_error "$helper" 1 'The clipboard request was invalid.' 'invalid JSON' write
check_error "$helper" 1 'There are no available formats.' '[]' choose

desktop="$run_dir/desktop/copied.md.app"
mkdir -p "$desktop/Contents/MacOS" "$desktop/Contents/Helpers" "$desktop/Contents/Resources"
cp "$root_dir/Info.plist" "$desktop/Contents/"
ln -s "$(command -v node)" "$desktop/Contents/Helpers/node"
cp "$COPIED_MD_CLI_OUT" "$desktop/Contents/Resources/converter.mjs"
executable=$desktop/Contents/MacOS/copied-md
compile "$root_dir/Pasteboard.swift" "$root_dir/GlobalHotKey.swift" \
  "$root_dir/FormatPicker.swift" "$root_dir/FormatIcons.swift" \
  "$root_dir/ConverterRuntime.swift" "$root_dir/DesktopApp.swift" -o "$executable"
LLVM_PROFILE_FILE="$run_dir/desktop.profraw" "$executable" --self-test

export LLVM_PROFILE_FILE="$run_dir/desktop-error-%p.profraw"
entrypoint="$desktop/Contents/Resources/converter.mjs"
printf '%s\n' 'console.log(JSON.stringify([{ id: "plain-text", label: "Plain Text" }]))' > "$entrypoint"
check_error "$executable" 1 'The bundled converter did not list Rich Text.' '' --self-test

cat > "$entrypoint" <<'JS'
if (process.argv[2] === 'targets') {
  console.log(JSON.stringify([{ id: 'rich-text', label: 'Rich Text' }]))
} else {
  console.log(JSON.stringify({
    schemaVersion: 1, target: 'rich-text', representations: { 'text/plain': 'Café' }, diagnostics: [],
  }))
}
JS
check_error "$executable" 1 'The bundled converter did not preserve Markdown or URLs.' '' --self-test

xcrun llvm-profdata merge -sparse "$run_dir"/*.profraw -o "$run_dir/native.profdata"
coverage() {
  action=$1
  shift
  xcrun llvm-cov "$action" "$run_dir/native-tests" -instr-profile="$run_dir/native.profdata" \
    -object "$helper" -object "$executable" -ignore-filename-regex='/macos/tests/' "$@"
}

coverage report
coverage export -summary-only > "$run_dir/coverage.json"
node --input-type=module - "$root_dir" "$run_dir/coverage.json" <<'JS'
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

const [, , root, report] = process.argv;
const { data } = JSON.parse(readFileSync(report, 'utf8'));
const files = new Map(data.flatMap(({ files }) => files.map(({ filename, summary }) => [filename, summary.lines])));
const sources = readdirSync(root, { recursive: true }).filter(
  (file) => file.endsWith('.swift') && !file.startsWith('tests/'),
);
for (const source of sources) {
  const lines = files.get(resolve(root, source));
  if (!lines?.count || lines.covered !== lines.count) {
    const detail = lines?.count ? `${lines.covered}/${lines.count} lines covered` : 'no instrumented lines';
    console.error(`${source}: ${detail}`);
    process.exitCode = 1;
  }
}
if (!process.exitCode) console.log('Every production Swift file has 100% line coverage.');
JS
