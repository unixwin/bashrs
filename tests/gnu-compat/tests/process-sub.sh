#!/usr/bin/env bash
# Test: process-sub
diff <(echo hello) <(echo hello) && echo same || echo diff
