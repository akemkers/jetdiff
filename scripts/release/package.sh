#!/bin/sh
# Bundles the binary and packs it with its checksum, the files a release has.
#
# usage: scripts/release/package.sh <binary> <version>
set -eu

archive=jetdiff-aarch64-apple-darwin.tar.gz
"$(dirname "$0")/bundle.sh" "$1" "$2" dist
tar -czf "$archive" -C dist jetdiff.app
shasum -a 256 "$archive" >"$archive.sha256"
