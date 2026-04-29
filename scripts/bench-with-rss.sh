#!/usr/bin/env bash
# Run the lifegame-core bench with peak RSS captured via /usr/bin/time -l.
#
# Usage:
#   ./scripts/bench-with-rss.sh           # full criterion run
#   ./scripts/bench-with-rss.sh --quick   # quick mode
#
# Outputs criterion's normal report to stdout and a summary line at the end:
#   peak RSS = <bytes> (<human>)
#
# IMPORTANT: must be run OUTSIDE the Claude Code sandbox. Inside the sandbox
# `/usr/bin/time -l` cannot read `kern.clockrate` and silently drops its
# detailed report (only real/user/sys lines survive), so peak RSS will not
# be parseable. For criterion-only runs without RSS, just call
# `cargo bench -p lifegame-core --bench next_generation` directly.
#
# macOS-specific: parses BSD `/usr/bin/time -l` output. On Linux you'd use
# `/usr/bin/time -v` instead and grep for "Maximum resident set size".

set -euo pipefail

cd "$(dirname "$0")/.."

# Keep temp file under cwd so sandboxed runs can write to it.
mkdir -p target/bench-tmp
LOG=$(mktemp target/bench-tmp/rss.XXXXXX)
trap 'rm -f "$LOG"' EXIT

# Run criterion through /usr/bin/time -l, capturing stderr (where -l writes
# its summary) to a file, then echoing it so the user still sees it.
# Avoid bash process substitution because it requires opening /dev/fd/* which
# is blocked inside the sandbox.
/usr/bin/time -l cargo bench -p lifegame-core --bench next_generation -- "$@" 2>"$LOG"
status=$?
cat "$LOG" >&2
if [[ $status -ne 0 ]]; then
    exit $status
fi

# Extract maximum resident set size (bytes on Apple Silicon, sometimes pages
# on older macOS). Heuristic: the line looks like
#   12345678  maximum resident set size
RSS_BYTES=$(grep -E 'maximum resident set size' "$LOG" | awk '{print $1}' | tail -1 || true)

if [[ -z "${RSS_BYTES}" ]]; then
    echo "WARN: could not parse max RSS from /usr/bin/time output" >&2
    exit 0
fi

# Human-readable
human() {
    local n=$1
    awk -v n="$n" 'BEGIN {
        if (n >= 1073741824) printf "%.2f GiB", n/1073741824;
        else if (n >= 1048576) printf "%.2f MiB", n/1048576;
        else if (n >= 1024) printf "%.2f KiB", n/1024;
        else printf "%d B", n;
    }'
}

echo
echo "================================================================"
echo "peak RSS = ${RSS_BYTES} bytes ($(human "${RSS_BYTES}"))"
echo "================================================================"
