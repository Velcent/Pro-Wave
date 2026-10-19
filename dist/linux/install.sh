#!/bin/sh
# Installs Pro Wave for the current user (no root needed). Undo with: ./install.sh --uninstall
set -e
here=$(cd "$(dirname "$0")" && pwd)
bin="$HOME/.local/bin"; apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/scalable/apps"; mime="$HOME/.local/share/mime"
if [ "$1" = "--uninstall" ]; then
  rm -f "$bin/prowave" "$apps/prowave-studio.desktop" "$icons/prowave.svg" "$mime/packages/prowave.xml"
  echo "Pro Wave removed."
else
  mkdir -p "$bin" "$apps" "$icons" "$mime/packages"
  install -m 755 "$here/prowave" "$bin/prowave"
  install -m 644 "$here/prowave-studio.desktop" "$apps/"
  install -m 644 "$here/prowave.svg" "$icons/"
  install -m 644 "$here/prowave-mime.xml" "$mime/packages/prowave.xml"
  command -v update-mime-database >/dev/null && update-mime-database "$mime" || true
  command -v update-desktop-database >/dev/null && update-desktop-database "$apps" || true
  echo "Installed to $bin/prowave. Start it from your app menu or run: prowave"
  case ":$PATH:" in *":$bin:"*) ;; *) echo "Note: add $bin to your PATH to run it from a terminal." ;; esac
fi
