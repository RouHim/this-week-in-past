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

CURRENT_ARCH=$(uname -m)

if [ "$CURRENT_ARCH" = "armv7l" ]; then
  CURRENT_ARCH="armv7"
fi

echo "Current arch is: $CURRENT_ARCH"

find . -wholename "*release/${1}" -type f | while read arch_binary; do
  echo "Checking: $arch_binary"
  if [[ "$arch_binary" = *"$CURRENT_ARCH"* ]]; then
    echo " -> Binary for this cpu arch is: $arch_binary"
    file "$arch_binary"
    cp "$arch_binary" "${1}"
    exit 0
  else
    echo " -> No match"
  fi
done
