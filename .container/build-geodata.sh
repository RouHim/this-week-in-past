#!/usr/bin/env bash
#
# Description:
#   Builds the merged place dataset used for offline city and district resolution.
#   Downloads the upstream GeoNames dumps (cities500.zip and allCountries.zip) and
#   merges them into one tab-separated file with a header line: every cities500 row
#   plus every allCountries PPLX (section/quarter) row, deduplicated by geonameid and
#   projected to the seven columns the app reads. Alternatively validates an existing
#   dataset instead of building one.
#
# Parameter:
#   $1 - Output path for the dataset, or --validate followed by a dataset file to check
#
# Example:
#   ./build-geodata.sh /tmp/geodata.txt
#   ./build-geodata.sh --validate /tmp/geodata.txt
#
# # # #

set -euo pipefail

BASE_URL="https://download.geonames.org/export/dump"
HEADER=$'#twip-places-v1\tname\tlatitude\tlongitude\tfeature_class\tfeature_code\tcountry_code\tpopulation'
MIN_ROWS=390000

WORK_DIR=""

cleanup() {
  if [ -n "$WORK_DIR" ]; then
    rm -rf "$WORK_DIR"
  fi
}
trap cleanup EXIT

validate() {
  local file="$1"

  if [ ! -s "$file" ]; then
    echo "dataset missing or empty: $file" >&2
    exit 1
  fi

  local first_line
  first_line=$(head -n 1 "$file")
  if [ "$first_line" != "$HEADER" ]; then
    echo "dataset header missing or wrong: $file" >&2
    exit 1
  fi

  local rows
  rows=$(( $(wc -l < "$file") - 1 ))
  if [ "$rows" -lt "$MIN_ROWS" ]; then
    echo "dataset too small: $file has $rows rows, expected at least $MIN_ROWS" >&2
    exit 1
  fi

  local bad_rows
  bad_rows=$(awk -F'\t' 'NR > 1 && NF != 7' "$file" | wc -l)
  if [ "$bad_rows" -ne 0 ]; then
    echo "dataset has $bad_rows rows with other than 7 columns: $file" >&2
    exit 1
  fi

  if ! awk -F'\t' '$1 == "Longerich" && $5 == "PPLX" && $6 == "DE" { found = 1 } END { exit !found }' "$file"; then
    echo "dataset anchor row missing (Longerich PPLX DE): $file" >&2
    exit 1
  fi

  if ! grep -q "$(printf 'K\303\270benhavn')" "$file"; then
    echo "dataset UTF-8 check failed (København missing): $file" >&2
    exit 1
  fi

  echo "dataset ok: $rows rows, $(wc -c < "$file") bytes"
}

build() {
  local out="$1"
  WORK_DIR=$(mktemp -d)

  curl -fL --retry 3 --retry-delay 2 --connect-timeout 15 "$BASE_URL/cities500.zip" -o "$WORK_DIR/cities500.zip"
  curl -fL --retry 3 --retry-delay 2 --connect-timeout 15 "$BASE_URL/allCountries.zip" -o "$WORK_DIR/allCountries.zip"

  unzip -p "$WORK_DIR/cities500.zip" > "$WORK_DIR/cities500.txt"
  unzip -p "$WORK_DIR/allCountries.zip" | LC_ALL=C awk -F'\t' '$8 == "PPLX"' > "$WORK_DIR/pplx.txt"

  LC_ALL=C awk -F'\t' -v OFS='\t' '
    NR == FNR { seen[$1]++; print $2, $5, $6, $7, $8, $9, $15; next }
    !($1 in seen) { seen[$1]++; print $2, $5, $6, $7, $8, $9, $15 }
  ' "$WORK_DIR/cities500.txt" "$WORK_DIR/pplx.txt" > "$WORK_DIR/body.txt"

  { printf "%s\n" "$HEADER"; cat "$WORK_DIR/body.txt"; } > "$out"

  validate "$out"
}

case "${1:-}" in
  --validate)
    if [ -z "${2:-}" ]; then
      echo "usage: $0 --validate <file>" >&2
      exit 1
    fi
    validate "$2"
    ;;
  "")
    echo "usage: $0 <output-path> | --validate <file>" >&2
    exit 1
    ;;
  *)
    build "$1"
    ;;
esac
