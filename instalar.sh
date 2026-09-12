#!/usr/bin/env bash
# Instala mmmusic en el perfil del usuario y registra el lanzador de escritorio.
set -euo pipefail

cd "$(dirname "$0")"

cargo install --path . --locked

install -Dm644 mmmusic.desktop \
  "${XDG_DATA_HOME:-$HOME/.local/share}/applications/mmmusic.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${XDG_DATA_HOME:-$HOME/.local/share}/applications" || true
fi

echo "mmmusic instalado en ~/.cargo/bin/mmmusic"
echo "Asegúrate de tener ~/.cargo/bin en el PATH."
