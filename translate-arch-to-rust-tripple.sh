#!/usr/bin/env bash
#
# Description:
#   Translates the current cpu architecture to a rust triple.
#
# # # #

# Get input string
input_string=$1

# Define a function to convert the input string to a rust target triple
function to_triple() {
  case $1 in
    "x86_64-musl") echo "x86_64-unknown-linux-musl";;
    "aarch64-musl") echo "aarch64-unknown-linux-musl";;
    "armv7-musleabihf") echo "armv7-unknown-linux-musleabihf";;
    "arm-musleabihf") echo "arm-unknown-linux-musleabihf";;
    *) echo "Error: Unsupported input string" && return 1;;
  esac
}

# Call the function and save the output to a variable
output=$(to_triple "$input_string")

# check if the function returned an error
if [ $? -eq 1 ]; then
  exit 1
fi

# Print the output
echo "$output"
