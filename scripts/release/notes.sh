#!/bin/sh
# Writes the message of the annotated tag to stdout. It is the release notes.
#
# usage: scripts/release/notes.sh <tag>
set -eu

tag=$1
if [ "$(git cat-file -t "$tag")" != tag ]; then
  echo "$tag has no message. Make it with git tag -a, the message is the release notes" >&2
  exit 1
fi
git tag -l --format='%(contents)' "$tag"
