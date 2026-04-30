#!/usr/bin/env bash
# Verify that program-v2 contains no fee/admin surface.
#
# Reads scripts/fee-paths.txt and fails if:
#   - any PATH rule matches a file or directory that exists
#   - any SYMBOL rule (ERE regex) matches inside program/src/**/*.rs
#
# Designed to run in CI (Linux, GNU grep) and locally (macOS, BSD grep).
# Exit codes:
#   0 = clean
#   1 = violation found
#   2 = invalid invocation / missing fee-paths.txt

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
RULES_FILE="$REPO_ROOT/scripts/fee-paths.txt"
SOURCE_ROOT="$REPO_ROOT/program/src"

if [[ ! -f "$RULES_FILE" ]]; then
    echo "error: $RULES_FILE not found" >&2
    exit 2
fi

violations=0

# ---- PATH rules: file/dir must not exist ---------------------------------
while IFS= read -r line; do
    # Strip comments and trim
    line="${line%%#*}"
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "$line" ]] && continue

    if [[ "$line" == PATH\ * ]]; then
        target="${line#PATH }"
        if [[ -e "$REPO_ROOT/$target" ]]; then
            echo "✗ forbidden path exists: $target" >&2
            violations=$((violations + 1))
        fi
    fi
done < "$RULES_FILE"

# ---- SYMBOL rules: regex must not match in program/src/**.rs -------------
if [[ -d "$SOURCE_ROOT" ]]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        [[ -z "$line" ]] && continue

        if [[ "$line" == SYMBOL\ * ]]; then
            pattern="${line#SYMBOL }"
            # -E extended regex; -r recursive; --include filters; -l list files;
            # -n line numbers; -H show filename. Use -lE first to detect quickly.
            if matches=$(grep -rEn --include='*.rs' -- "$pattern" "$SOURCE_ROOT" 2>/dev/null); then
                echo "✗ forbidden symbol /$pattern/ matched:" >&2
                # Indent each match line for readability
                printf '%s\n' "$matches" | sed 's/^/    /' >&2
                violations=$((violations + 1))
            fi
        fi
    done < "$RULES_FILE"
fi

if (( violations > 0 )); then
    echo "" >&2
    echo "FAIL: $violations fee-surface violation(s) found." >&2
    echo "Either remove the offending file/symbol, or update scripts/fee-paths.txt" >&2
    echo "if the rule itself is wrong." >&2
    exit 1
fi

echo "✓ program-v2 source is fee-surface clean."
