#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

if ! command -v cloc >/dev/null 2>&1; then
    printf '%s\n' 'audit-rust-placeholders: cloc is required' >&2
    exit 1
fi

module_index=$(mktemp)
trap 'rm -f "$module_index"' EXIT
rg -n '^[[:space:]]*(pub[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+' src --glob '*.rs' |
    tr '\\' '/' > "$module_index" || true

printf 'path\tcode_zero\tmodule_status\tdisposition\n'

cloc --by-file --csv src |
tr '\\' '/' |
awk -F, -v module_index="$module_index" '
BEGIN {
    while ((getline line < module_index) > 0) {
        file = line
        sub(/:.*/, "", file)
        declaration = line
        sub(/^[^:]*:[0-9]+:/, "", declaration)
        if (declaration !~ /(^|[[:space:]])mod[[:space:]]+[A-Za-z0-9_]+/) {
            continue
        }
        sub(/^.*mod[[:space:]]+/, "", declaration)
        sub(/[[:space:]].*$/, "", declaration)
        if (file ~ /\/mod\.rs$/) {
            sub(/\/mod\.rs$/, "", file)
        } else if (file == "src/lib.rs" || file == "src/main.rs") {
            file = "src"
        } else {
            sub(/\.rs$/, "", file)
        }
        active[file "/" declaration ".rs"] = 1
    }
    close(module_index)
}
$1 == "Rust" && $5 == 0 {
    path = $2
    gsub(/\r/, "", path)
    if (path in active) {
        module_status = "active"
    } else {
        module_status = "unreferenced"
    }
    if (path ~ /^src\/(complete|history|input|locale|sys)\//) {
        disposition = "host-owned-or-deferred"
    } else {
        disposition = "duplicate-owner-candidate"
    }
    printf "%s\t0\t%s\t%s\n", path, module_status, disposition
}
'
