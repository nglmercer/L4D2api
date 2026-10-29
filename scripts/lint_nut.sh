#!/usr/bin/env bash
# Fail on banned patterns in the Squirrel sources: no cheats, no HTTP
# listeners, no dynamic script execution, no console-command smuggling.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0
ban() {
  if grep -RInE "$1" addon/ ; then
    echo "BANNED: $1" >&2
    fail=1
  fi
}

ban 'sv_cheats[[:space:]]+1'
ban 'HttpListener|HTTP|Socket|fetch\(|RunScriptFile|compilestring|eval'
ban 'SendToConsole|ServerCommand|ClientCommand|script_help'
ban '\.bat|\.sh|SourceMod|MetaMod|rcon|password'

# Every .nut must end with a newline and be ASCII-only.
while IFS= read -r f; do
  if LC_ALL=C grep -qP '[^\x00-\x7F]' "$f"; then echo "NON-ASCII: $f" >&2; fail=1; fi
  if [ -n "$(tail -c 1 "$f")" ]; then echo "NO TRAILING NEWLINE: $f" >&2; fail=1; fi
done < <(find addon -name '*.nut' -o -name 'addoninfo.txt')

# Required game paths must exist in the source tree.
for p in addoninfo.txt \
         scripts/vscripts/director_base_addon.nut \
         scripts/vscripts/tiktools_l4d2/bootstrap.nut \
         scripts/vscripts/tiktools_l4d2/protocol.nut \
         scripts/vscripts/tiktools_l4d2/state.nut \
         scripts/vscripts/tiktools_l4d2/actions.nut \
         scripts/vscripts/tiktools_l4d2/spawns.nut \
         scripts/vscripts/tiktools_l4d2/limits.nut ; do
  if [ ! -f "addon/$p" ]; then echo "MISSING: addon/$p" >&2; fail=1; fi
done

exit $fail
