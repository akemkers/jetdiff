#!/bin/sh
# Stops if the crate or the plugin doesn't have the version of the tag.
#
# usage: scripts/release/check-version.sh <version>
set -eu

version=$1
root=$(cd "$(dirname "$0")/../.." && pwd)
crate=$(cargo metadata --manifest-path "$root/Cargo.toml" --no-deps --format-version 1 |
  jq -r '.packages[] | select(.name == "jetdiff") | .version')
plugin=$(jq -r .version "$root/claude-plugin/.claude-plugin/plugin.json")
if [ "$crate" != "$version" ] || [ "$plugin" != "$version" ]; then
  echo "The tag is $version, the crate $crate and the plugin $plugin" >&2
  exit 1
fi
