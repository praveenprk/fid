#!/bin/sh
# Build the draft to text and HTML in spec/build/. Fails on any error
# from kramdown-rfc or xml2rfc, and on any kramdown-rfc warning unless
# KRAMDOWN_OFFLINE is set (offline, every reference is a placeholder).
set -eu
cd "$(dirname "$0")"
draft=draft-kumar-fid-00
out=build
mkdir -p "$out"
if ! kramdown-rfc "$draft.md" > "$out/$draft.xml" 2> "$out/kramdown.log"; then
    cat "$out/kramdown.log"
    exit 1
fi
cat "$out/kramdown.log"
if [ -z "${KRAMDOWN_OFFLINE:-}" ] && grep -q '^\*' "$out/kramdown.log"; then
    echo "spec/build.sh: kramdown-rfc reported problems (above)" >&2
    exit 1
fi
xml2rfc --text --html "$out/$draft.xml" -p "$out" 2>&1 | tee "$out/xml2rfc.log"
if grep -qi 'error' "$out/xml2rfc.log" || [ ! -s "$out/$draft.txt" ]; then
    echo "spec/build.sh: xml2rfc failed (above)" >&2
    exit 1
fi
echo "built $out/$draft.txt and $out/$draft.html"
