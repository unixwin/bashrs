B=9
( echo $((0 && B=42)); echo inner:$B )
echo outer:$B
