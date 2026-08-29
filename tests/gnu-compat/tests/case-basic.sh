#!/usr/bin/env bash
# Test: case-basic
x="hello"; case $x in hello) echo "matched" ;; *) echo "no" ;; esac
