#!/usr/bin/env bash
set -euo pipefail

names=(Dennis Toby Lion Jason)
url="https://www.random.org/sequences/?min=1&max=${#names[@]}&col=1&format=plain&rnd=new"

if ! command -v curl >/dev/null 2>&1; then
  echo "Error: curl is required." >&2
  exit 1
fi

response="$(curl -fsSL \
  --max-time 180 \
  --user-agent "SDR-random-assign/1.0" \
  "$url")"

if [[ "$response" == Error:* ]]; then
  echo "$response" >&2
  exit 1
fi

sequence=()
while IFS= read -r value; do
  [[ -n "$value" ]] && sequence+=("$value")
done <<< "$response"

if [[ "${#sequence[@]}" -ne "${#names[@]}" ]]; then
  echo "Error: expected ${#names[@]} sequence values, got ${#sequence[@]}." >&2
  exit 1
fi

for position in "${!sequence[@]}"; do
  name_index=$((sequence[position] - 1))

  if (( name_index < 0 || name_index >= ${#names[@]} )); then
    echo "Error: random.org returned an out-of-range value: ${sequence[position]}" >&2
    exit 1
  fi

  printf '%d: %s\n' "$((position + 1))" "${names[name_index]}"
done
