#!/usr/bin/env bash
# case-16-glob — glob 特性 (对照, 应 PASS)
cd "$HOME/.oh-my-winuxsh/themes"
echo "A: $(for f in a*.toml; do printf '%s ' "$f"; done; echo)"
echo "B: $(for f in *.toml; do printf '%s ' "$f"; done | head -c 40; echo)"
shopt -s nullglob
echo "C: $(for f in zzz*.none; do printf '%s' "$f"; done; echo "end")"
shopt -u nullglob
echo "D: $(for f in zzz*.none; do printf '%s' "$f"; done; echo "end")"
