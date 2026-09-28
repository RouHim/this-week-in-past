#!/usr/bin/env bash
set -euo pipefail

# Uploads one file as an asset to the latest GitHub release.
#
# Parameter:
#   $1 - GitHub token with `repo` scope
#   $2 - Path of the file to upload
#   $3 - Asset name on the release
#
# Example:
#   ./upload-asset-to-release.sh "$TOKEN" ./target/release/this-week-in-past this-week-in-past-x86_64
#
# # # #

TOKEN=$1
FILE_PATH=$2
NAME=$3
REPO="RouHim/this-week-in-past"

echo "Uploading ${FILE_PATH} to ${NAME}"

# The lookup must be authenticated: unauthenticated api.github.com calls from
# shared runner IPs hit the rate limit, which yields no id and a 404 upload.
RELEASE_ID=$(curl -sS --fail-with-body -H "Authorization: token ${TOKEN}" \
  "https://api.github.com/repos/${REPO}/releases/latest" | jq -r '.id')
if ! [[ "${RELEASE_ID}" =~ ^[0-9]+$ ]]; then
  echo "could not resolve the latest release id of ${REPO} (got '${RELEASE_ID}')" >&2
  exit 1
fi

# Replace an asset with the same name so re-runs stay idempotent instead of
# failing with 422 "already_exists".
EXISTING_ASSET_ID=$(curl -sS --fail-with-body -H "Authorization: token ${TOKEN}" \
  "https://api.github.com/repos/${REPO}/releases/${RELEASE_ID}/assets" |
  jq -r --arg name "${NAME}" '.[] | select(.name == $name) | .id')
if [[ "${EXISTING_ASSET_ID}" =~ ^[0-9]+$ ]]; then
  curl -sS --fail-with-body -X DELETE -H "Authorization: token ${TOKEN}" \
    "https://api.github.com/repos/${REPO}/releases/assets/${EXISTING_ASSET_ID}"
fi

curl -sS --fail-with-body -X POST \
  -H "Content-Type: $(file -b --mime-type "${FILE_PATH}")" \
  -H "Authorization: token ${TOKEN}" \
  -T "${FILE_PATH}" \
  "https://uploads.github.com/repos/${REPO}/releases/${RELEASE_ID}/assets?name=${NAME}"

echo "uploaded ${NAME} to release ${RELEASE_ID}"
