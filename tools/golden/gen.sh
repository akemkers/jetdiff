#!/usr/bin/env bash
# Builds the golden generator and writes fixtures/*/expected.json.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
fixtures="$(cd "$here/../../fixtures" && pwd)"

gradle -p "$here" --console=plain -q installDist

cases=()
for dir in "$fixtures"/*/; do
  dir="${dir%/}"
  if [[ -f "$dir/a.txt" && -f "$dir/b.txt" ]]; then
    cases+=("$dir")
  fi
done

if [[ ${#cases[@]} -eq 0 ]]; then
  echo "no fixtures found in $fixtures" >&2
  exit 1
fi

"$here/build/install/jetdiff-golden/bin/jetdiff-golden" "${cases[@]}"
