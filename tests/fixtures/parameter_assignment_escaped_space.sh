unset v
printf '<%s> ' "${v=a\ b}" x "${v=c\ d}"; echo
