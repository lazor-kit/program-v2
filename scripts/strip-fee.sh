#!/usr/bin/env bash
# Post-cherry-pick cleanup: remove fee/admin surface that may have been
# introduced by an upstream patch from lazorkit-protocol.
#
# Workflow:
#   1. Apply the upstream patch:    git am --3way path/to/feature.mbox
#                              or:  git apply path/to/feature.patch && git add -A
#   2. Run this script:             ./scripts/strip-fee.sh
#   3. Hand-resolve any remaining symbol leaks reported below.
#   4. Verify:                      ./scripts/check-no-fee.sh
#   5. Commit the cleaned result.
#
# Behavior:
#   - PATH rules are auto-removed via `git rm -rf` (when present).
#   - SYMBOL rules are NOT auto-edited — stripping a struct/function reference
#     from the middle of a file requires understanding context. The script
#     prints each offending file:line and exits non-zero so the user notices.
#
# Use --dry-run to see what would happen without modifying anything.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
RULES_FILE="$REPO_ROOT/scripts/fee-paths.txt"
SOURCE_ROOT="$REPO_ROOT/program/src"

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=1
fi

if [[ ! -f "$RULES_FILE" ]]; then
    echo "error: $RULES_FILE not found" >&2
    exit 2
fi

cd "$REPO_ROOT"

removed=0
symbol_leaks=0

# ---- PATH rules: auto-remove via git rm ----------------------------------
echo "→ Removing forbidden paths..."
while IFS= read -r line; do
    line="${line%%#*}"
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "$line" ]] && continue

    if [[ "$line" == PATH\ * ]]; then
        target="${line#PATH }"
        if [[ -e "$target" ]]; then
            if (( DRY_RUN )); then
                echo "  [dry-run] would remove: $target"
            else
                # Use git rm if tracked, otherwise plain rm. -f ignores missing.
                if git ls-files --error-unmatch -- "$target" >/dev/null 2>&1; then
                    git rm -rf --quiet -- "$target"
                else
                    rm -rf -- "$target"
                fi
                echo "  removed: $target"
            fi
            removed=$((removed + 1))
        fi
    fi
done < "$RULES_FILE"

if (( removed == 0 )); then
    echo "  (no forbidden paths present)"
fi

# ---- SYMBOL rules: report only, don't edit -------------------------------
echo ""
echo "→ Scanning for forbidden symbols in program/src/**.rs..."
if [[ -d "$SOURCE_ROOT" ]]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        [[ -z "$line" ]] && continue

        if [[ "$line" == SYMBOL\ * ]]; then
            pattern="${line#SYMBOL }"
            if matches=$(grep -rEn --include='*.rs' -- "$pattern" "$SOURCE_ROOT" 2>/dev/null); then
                echo "  ✗ /$pattern/ leaked into source:"
                printf '%s\n' "$matches" | sed 's/^/      /'
                symbol_leaks=$((symbol_leaks + 1))
            fi
        fi
    done < "$RULES_FILE"
fi

echo ""
if (( symbol_leaks > 0 )); then
    echo "FAIL: $symbol_leaks symbol leak(s) require manual cleanup." >&2
    echo "Edit the files above to remove fee references, then re-run check-no-fee.sh." >&2
    exit 1
fi

if (( DRY_RUN )); then
    echo "✓ dry-run complete. $removed path(s) would be removed."
else
    echo "✓ strip complete. $removed path(s) removed, no symbol leaks."
    echo "  Run ./scripts/check-no-fee.sh to verify."
fi
