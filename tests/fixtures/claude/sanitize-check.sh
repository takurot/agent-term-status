#!/bin/bash
# Privacy check for the Claude Code hook fixture corpus (I-04, SPEC §14.2).
# Fails if any fixture leaks a real home directory, machine temp path,
# unexpanded home shorthand, or API-key-shaped string.
set -euo pipefail

dir="$(cd "$(dirname "$0")" && pwd)"
status=0

while IFS= read -r -d '' file; do
    leaks="$(grep -oE '/Users/[A-Za-z0-9._-]+' "$file" | grep -v '^/Users/testuser$' || true)"
    if [ -n "$leaks" ]; then
        echo "LEAK (home dir) in $file: $leaks"
        status=1
    fi
    if grep -qE '(/private)?/var/folders/' "$file"; then
        echo "LEAK (temp path) in $file"
        status=1
    fi
    if grep -qE '"~/|~/' "$file"; then
        echo "LEAK (home shorthand) in $file"
        status=1
    fi
    if grep -qE 'sk-(ant|proj)-[A-Za-z0-9_-]+' "$file"; then
        echo "LEAK (API key) in $file"
        status=1
    fi
done < <(find "$dir" -name '*.json' -print0)

if [ "$status" -eq 0 ]; then
    echo "sanitize-check: OK ($(find "$dir" -name '*.json' | wc -l | tr -d ' ') fixtures)"
fi
exit "$status"
