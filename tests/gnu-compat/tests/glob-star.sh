#!/usr/bin/env bash
# Test: glob-star
echo *.nonexistent 2>/dev/null || echo none
