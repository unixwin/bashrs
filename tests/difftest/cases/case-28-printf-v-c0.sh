#!/usr/bin/env bash
# case-28-printf-v-c0 - printf -v preserves command substitution and C0 bytes
printf -v from_sub 'sub:%s' "$(printf '%s' value)"
printf '%s\n' "$from_sub"

format=$'fmt:\001%s\002'
argument=$'arg\003'
printf -v with_c0 "$format" "$argument"
printf '%s' "$with_c0" | od -An -t x1

declare -A assoc
printf -v "assoc[$'key\004part']" '%s' stored
printf '%s' "${assoc[$'key\004part']}" | od -An -t x1
