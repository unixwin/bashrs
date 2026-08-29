#!/usr/bin/env bash
a=4
echo "${a#'$('\\'}"
echo "${a-'$('\\'}"
echo "${a+'$('\\'}"
