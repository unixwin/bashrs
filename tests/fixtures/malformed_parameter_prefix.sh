unset a
printf '%s\n' ${a:=a\ b}
echo "$a"
foo=ba
echo "${foo:-"a}"
