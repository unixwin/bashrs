#!/usr/bin/env bash
# Test: while-loop
i=0; while [[ $i -lt 3 ]]; do echo -n "$i "; i=$((i+1)); done; echo
