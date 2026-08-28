#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
package_dir="$project_root/packaging/flatpak"
deb_path="$(find "$project_root/src-tauri/target/release/bundle/deb" -maxdepth 1 -type f -name '*.deb' -print -quit)"

if [[ -z "$deb_path" ]]; then
  echo "No Tauri Debian package was found." >&2
  exit 1
fi

cp "$deb_path" "$package_dir/everquest-loot-tracker.deb"

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

flatpak-builder --user --force-clean --install-deps-from=flathub --default-branch=stable --repo="$project_root/flatpak-repo" "$project_root/flatpak-build" "$package_dir/com.eqtools.loottracker.yml"

version="$(node -p "require('$project_root/package.json').version")"
bundle="$project_root/EverQuest-Loot-Tracker-${version}-x86_64.flatpak"

flatpak build-bundle "$project_root/flatpak-repo" "$bundle" com.eqtools.loottracker stable --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo

echo "Built $bundle"
