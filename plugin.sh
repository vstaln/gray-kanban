#!/bin/sh
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
sys="$(uname -s)"
case "$sys" in
MINGW* | MSYS* | CYGWIN*) rel="x86_64-pc-windows-msvc/gray-kanban.exe" ;;
Darwin*)
    case "$(uname -m)" in
    arm64) rel="aarch64-apple-darwin/gray-kanban" ;;
    *) rel="x86_64-apple-darwin/gray-kanban" ;;
    esac ;;
*)
    case "$(uname -m)" in
    aarch64 | arm64) rel="aarch64-unknown-linux-musl/gray-kanban" ;;
    *) rel="x86_64-unknown-linux-musl/gray-kanban" ;;
    esac ;;
esac
if [ -x "$here/$rel" ]; then exec "$here/$rel" "$@"; fi
for cand in "$here/target/release/gray-kanban.exe" "$here/target/release/gray-kanban" "$here/target/debug/gray-kanban.exe" "$here/target/debug/gray-kanban"; do
    if [ -x "$cand" ]; then exec "$cand" "$@"; fi
done
echo "gray-kanban binary not found in $here" >&2
exit 1
