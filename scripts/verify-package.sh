#!/bin/sh
# Inspect an image without launching its executable. Prints its SHA-256 on success.
set -eu

if [ "$#" != 2 ]; then
    echo "usage: verify-package.sh DMG NUMERIC_VERSION" >&2
    exit 1
fi
image="$1"
version="$2"
if ! printf '%s\n' "$version" | LC_ALL=C grep -Eq '^(0|[1-9][0-9]*)\.([1-9]|1[0-2])\.(0|[1-9][0-9]*)$' ||
    [ "$(printf '%s' "$version" | wc -l | tr -d ' ')" != 0 ]; then
    echo "invalid version" >&2
    exit 1
fi
[ "$(uname -s)" = Darwin ] || { echo "DMG verification requires macOS" >&2; exit 1; }
hdiutil verify "$image" >/dev/null
scratch=$(mktemp -d "${TMPDIR:-/tmp}/regie-verify.XXXXXX")
mounted=false
cleanup() {
    result=$?
    trap - EXIT HUP INT TERM
    if [ "$mounted" = true ]; then
        hdiutil detach -quiet "$scratch/mount" || result=1
    fi
    rmdir "$scratch/mount" "$scratch" 2>/dev/null || true
    exit "$result"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
mkdir "$scratch/mount"
hdiutil attach -quiet -readonly -nobrowse -mountpoint "$scratch/mount" "$image"
mounted=true

python3 - "$scratch/mount" "$version" <<'PY'
from pathlib import Path
import plistlib
import sys

root, version = Path(sys.argv[1]), sys.argv[2]
app = root / 'Regie.app'
try:
    with (app / 'Contents/Info.plist').open('rb') as stream:
        info = plistlib.load(stream)
    for key in ('CFBundleVersion', 'CFBundleShortVersionString'):
        if info.get(key) != version:
            raise ValueError(f'{key} version does not match {version}')
    for key, expected in {'CFBundleExecutable': 'regie', 'CFBundleIdentifier': 'com.jonathantobe.regie',
                          'CFBundlePackageType': 'APPL', 'CFBundleIconFile': 'Regie',
                          'LSUIElement': True}.items():
        if info.get(key) != expected:
            raise ValueError(f'incorrect {key}')
    icon = app / 'Contents/Resources/Regie.icns'
    if not icon.is_file() or icon.read_bytes()[:4] != b'icns':
        raise ValueError('missing or invalid app icon')
    link = root / 'Applications'
    if not link.is_symlink() or str(link.readlink()) != '/Applications':
        raise ValueError('Applications must link to /Applications')
except (OSError, ValueError, plistlib.InvalidFileException) as error:
    sys.exit(str(error))
PY
binary="$scratch/mount/Regie.app/Contents/MacOS/regie"
[ -x "$binary" ] || { echo "missing executable" >&2; exit 1; }
[ "$(lipo -archs "$binary")" = arm64 ] || { echo "expected arm64 executable" >&2; exit 1; }
codesign --verify --deep --strict "$scratch/mount/Regie.app"
codesign -dv "$scratch/mount/Regie.app" 2>&1 | grep -q '^Signature=adhoc$'
shasum -a 256 "$image"
