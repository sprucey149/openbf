#!/usr/bin/env bash
# scripts/install_linux.sh -- OpenBF's own icon and application entry in the user's
# session, so the dock, the taskbar and the file manager show it (branding/).
#
#   scripts/install_linux.sh [--exec <path/to/openbf>] [--prefix <dir>]
#
# --exec defaults to dist/openbf, the file scripts/pack.py writes, and --prefix to
# $HOME/.local/share. The entry goes to <prefix>/applications/openbf.desktop with
# @EXE@ filled in, and every size to
# <prefix>/icons/hicolor/<size>/apps/openbf.png. The desktop and icon caches are
# refreshed when the tools are installed. The entry's file name, the packed game's
# name and StartupWMClass are all "openbf", which is what a Wayland session matches
# an app id against and what an X11 class check looks for.
# Exit: 0 installed, 1 an input is missing.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="$HOME/.local/share"
EXEC=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --exec) EXEC="$2"; shift 2 ;;
    --prefix) PREFIX="$2"; shift 2 ;;
    -h|--help) sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "install_linux: unknown argument: $1 (try --help)" >&2; exit 1 ;;
  esac
done
[[ -z "$EXEC" ]] && EXEC="$ROOT/dist/openbf"
[ -f "$EXEC" ] || { echo "install_linux: $EXEC is not there: run python3 scripts/pack.py" >&2; exit 1; }
[ -f "$ROOT/branding/linux/openbf.desktop.template" ] || { echo "install_linux: the desktop template is missing: run python3 scripts/make_icons.py" >&2; exit 1; }
[ -d "$ROOT/branding/linux/hicolor" ] || { echo "install_linux: branding/linux/hicolor is missing: run python3 scripts/make_icons.py" >&2; exit 1; }

mkdir -p "$PREFIX/applications"
sed "s|@EXE@|$EXEC|" "$ROOT/branding/linux/openbf.desktop.template" > "$PREFIX/applications/openbf.desktop"
chmod 644 "$PREFIX/applications/openbf.desktop"

sizes=0
for dir in "$ROOT/branding/linux/hicolor"/*; do
  size="$(basename "$dir")"
  src="$dir/apps/openbf.png"
  [ -f "$src" ] || continue
  dest="$PREFIX/icons/hicolor/$size/apps"
  mkdir -p "$dest"
  cp "$src" "$dest/openbf.png"
  touch "$dest"   # the icon-theme spec wants the folder's mtime to change
  sizes=$((sizes + 1))
done

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$PREFIX/applications" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -t "$PREFIX/icons/hicolor" || true
echo "install_linux: entry + $sizes icon size(s) under $PREFIX (Exec=$EXEC)"
echo "If the icon does not appear, log out and in: a session reads these caches at login."
