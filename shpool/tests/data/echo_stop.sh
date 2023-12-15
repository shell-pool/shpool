#!/bin/bash

echo "$1"

echo "$0"

while read -r line;
do
  if [[ "$line" == "stop" ]] ; then
    exit
  fi
done
