#!/bin/bash
cd /mnt/d/repo/rubash/target/upstream-tests
export PATH=/tmp/bash-test-env:$PATH
export THIS_SH=bash
for name in dynvar; do
  echo "Generating $name.right..."
  bash ./$name.tests > /mnt/d/repo/rubash/tests/gnu-compat/upstream-rights/$name.right 2>&1
  echo "  $(wc -l < /mnt/d/repo/rubash/tests/gnu-compat/upstream-rights/$name.right) lines"
done
