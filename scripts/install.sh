#!/bin/sh
# Installs or updates jetdiff on macOS with Apple Silicon, and the Claude Code plugin if claude
# is on the PATH. Running it again changes nothing, unless a newer version is out.
#
#   curl -fsSL https://github.com/akemkers/jetdiff/releases/latest/download/install.sh | sh
#
# JETDIFF_VERSION=v0.2.0    that version instead of the newest, also an older one
# JETDIFF_BIN=<folder>      where the jetdiff command goes, ~/.local/bin by default
# JETDIFF_HOME=<folder>     where jetdiff.app goes, ~/.local/share/jetdiff by default
# JETDIFF_CLAUDE=<command>  claude with any arguments it needs, or "no" to skip the plugin
set -eu

releases=${JETDIFF_RELEASES:-https://github.com/akemkers/jetdiff/releases}
bin_dir=${JETDIFF_BIN:-$HOME/.local/bin}
home=${JETDIFF_HOME:-$HOME/.local/share/jetdiff}
claude=${JETDIFF_CLAUDE:-claude}
app=$home/jetdiff.app
executable=$app/Contents/MacOS/jetdiff
command=$bin_dir/jetdiff
archive=jetdiff-aarch64-apple-darwin.tar.gz

say() { printf '%s\n' "$*"; }
fail() {
  printf 'jetdiff: %s\n' "$*" >&2
  exit 1
}

# True if version $1 is newer than $2. Compared as numbers, so 0.10.0 is newer than 0.9.0.
newer() {
  [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -t. -k1,1n -k2,2n -k3,3n | tail -n 1)" = "$1" ]
}

install_app() {
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  say "Downloading jetdiff $version"
  curl -fsSL -o "$tmp/$archive" "$releases/download/$tag/$archive"
  curl -fsSL -o "$tmp/$archive.sha256" "$releases/download/$tag/$archive.sha256"
  (cd "$tmp" && shasum -a 256 -c "$archive.sha256" >/dev/null) || fail "the download doesn't match its checksum"
  tar -xzf "$tmp/$archive" -C "$tmp"

  # Swapped in at the end, so a failed download leaves the old version as it was
  mkdir -p "$home"
  rm -rf "$app.new" "$app.old"
  mv "$tmp/jetdiff.app" "$app.new"
  if [ -d "$app" ]; then mv "$app" "$app.old"; fi
  mv "$app.new" "$app"
  rm -rf "$app.old"
}

install_command() {
  shim=$(printf '#!/bin/sh\nexec "%s" "$@"' "$executable")
  if [ "$(cat "$command" 2>/dev/null)" = "$shim" ]; then return; fi
  mkdir -p "$bin_dir"
  printf '%s\n' "$shim" >"$command.new"
  chmod +x "$command.new"
  mv "$command.new" "$command"
}

# stdin is this script under curl | sh, so claude gets none. $claude is split on purpose.
run_claude() { $claude "$@" </dev/null; }

# The installed plugin's version, empty if it isn't installed. JavaScript for Automation reads
# the JSON, since every Mac has it and not every Mac has jq.
plugin_version() {
  json=$(run_claude plugin list --json 2>/dev/null) || return 0
  osascript -l JavaScript -e 'function run(argv) {
    const plugin = JSON.parse(argv[0]).find((p) => p.id === "jetdiff@jetdiff")
    return plugin ? plugin.version : ""
  }' "$json" 2>/dev/null || true
}

# Compares with the release, not with what this run did, so a plugin that failed to update
# last time is updated now.
install_plugin() {
  plugin=$(plugin_version)
  if [ -z "$plugin" ]; then
    run_claude plugin marketplace add akemkers/jetdiff && run_claude plugin install jetdiff@jetdiff
  elif newer "$version" "$plugin"; then
    run_claude plugin marketplace update jetdiff && run_claude plugin update jetdiff@jetdiff
  fi
}

[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] ||
  fail "there is only a prebuilt jetdiff for macOS with Apple Silicon. The README says how to build it."

# Never write over a jetdiff that this script didn't put there
if [ -e "$command" ] && ! grep -qF "$executable" "$command" 2>/dev/null; then
  fail "$command is there already, and not from this script. Remove it, or pick another folder with JETDIFF_BIN."
fi

installed=
if [ -f "$app/Contents/Info.plist" ]; then
  installed=$(plutil -extract CFBundleShortVersionString raw "$app/Contents/Info.plist")
fi
tag=${JETDIFF_VERSION:-$(basename "$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$releases/latest")")}
case $tag in
  v[0-9]*) ;;
  *) fail "found no release of jetdiff" ;;
esac
version=${tag#v}

if [ -z "$installed" ] || newer "$version" "$installed" ||
  { [ -n "${JETDIFF_VERSION:-}" ] && [ "$version" != "$installed" ]; }; then
  install_app
  say "Installed jetdiff $version"
else
  say "jetdiff $installed is installed and up to date"
fi
install_command

if [ "$claude" = no ]; then
  :
elif command -v "${claude%% *}" >/dev/null 2>&1; then
  install_plugin || say "Couldn't install the Claude Code plugin. In Claude Code: /plugin install jetdiff --marketplace akemkers/jetdiff"
else
  say "Claude Code isn't on the PATH, so the plugin is left out"
fi

case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) say "Add $bin_dir to your PATH to run jetdiff" ;;
esac
found=$(command -v jetdiff || true)
if [ -n "$found" ] && [ "$found" != "$command" ]; then
  say "$found comes first on your PATH, so that is the jetdiff that runs"
fi
