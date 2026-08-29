#!/usr/bin/env bash
# Test: set-e
set -e; false || true; echo survived
