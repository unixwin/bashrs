#!/usr/bin/env bash
# case-26: alias 语义（单引号防御、管道中 alias、unalias）— 族 H 回归
shopt -s expand_aliases
alias hi="echo hello"
hi
'hi' || echo "quoted-not-expanded"
alias pipehi="echo pipehi"
pipehi | cat
unalias pipehi 2>/dev/null
hi
