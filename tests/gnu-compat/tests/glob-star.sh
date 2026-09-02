#!/usr/bin/env bash
# Test: glob-sta
echo *.nonexistent 2>/dev/null || echo none
