#!/bin/env bash
set -e

GITHUB_TOKEN=$1
BIN_PATH=$2
NAME=$3
echo "Uploading $BIN_PATH to $NAME"
RELEASE_ID=$(curl -s https://api.github.com/repos/RouHim/this-week-in-past/releases/latest | jq -r '.id' )
UPLOAD_URL="https://uploads.github.com/repos/RouHim/this-week-in-past/releases/${RELEASE_ID}/assets?name=${NAME}"
curl -X POST \
  -H "Content-Type: $(file -b --mime-type "$BIN_PATH")" \
	-H "Authorization: token ${GITHUB_TOKEN}"\
	-T "${BIN_PATH}" \
  "${UPLOAD_URL}"