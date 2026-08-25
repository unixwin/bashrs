#!/usr/bin/env bash
# case-27-printf-percent-b - unknown %b escapes retain their backslash
printf '<%b>|<%b>\n' 'x\qy' 'a\z'
printf '<%b>\n' 'before\cafter'
printf 'after\n'
