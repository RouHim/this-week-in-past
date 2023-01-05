#!/usr/bin/env bash
#
# Description:
#   Translates the current cpu architecture to the a rust triple.
#
# # # #

CURRENT_ARCH=$(uname -m)

if [ "$CURRENT_ARCH" = "armv7l" ]; then
  CURRENT_ARCH="armv7"
fi

# Iterate all folders in the target folder
for arch_folder in $(find target -maxdepth 1 -type d); do
    # Check if the folder name contains the current arch
    if [[ "$arch_folder" = *"$CURRENT_ARCH"* ]]; then
      RUST_TRIPLE=$(echo "$arch_folder" | sed -e "s/.*\///g")
      echo "$RUST_TRIPLE"
      exit 0
    fi
done

exit 1