unset a
printf '%s\n' ${a:=a\ b}
echo "$a"
foo=bar
echo "${foo:-"a}"
