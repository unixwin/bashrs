#!/usr/bin/env bash
# Test: param-assign
unset x; echo ${x:=assigned} $x
