#!/usr/bin/env bash
#
# Description:
#   Stages the specified rust binary for the current cpu architecture
#
# Parameter:
#   $1 - Binary file name to stage
#
# Example:
#   ./stage-arch-bin.sh this-week-in-past
#
# # # #

if [ -n "$1" ]; then
  echo "Staging binary file arch: $1"
else
  echo "Binary file name to stage not supplied! First parameter is required."
  exit 1
fi

# TODO: fix for armv7 -> armv7l
CURRENT_ARCH=$(uname -m)
echo "Current arch is: $CURRENT_ARCH"

find . -wholename "*release/${1}" -type f | while read arch_binary; do
  echo "Checking: $arch_binary"
  if [[ "$arch_binary" = *"$CURRENT_ARCH"* ]]; then
    echo "Binary for this cpu arch is: $arch_binary"
    file "$arch_binary"
    cp "$arch_binary" "${1}"
    exit 0
  else
    echo " -> No match"
  fi
done

echo "No binary found for: $CURRENT_ARCH"
echo "List of available binaries:"
find . -wholename "*release/${1}" -type f
exit 1