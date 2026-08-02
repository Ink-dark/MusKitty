#!/bin/bash
# Clone path dependencies for independent CI.
#
# muskitty-cascade uses path dependencies for local development within the
# MusKitty workspace. When this repo is cloned standalone (e.g. on CI), the
# path deps must be materialized at the expected relative locations.
#
# Dependency chain:
#   muskitty-cascade → muskitty-css (path) → muskitty-css-parser (path) → muskitty-css-tokenizer (path)
#                    → muskitty-cssom (path) → muskitty-css (+ parser + tokenizer)
#                    → muskitty-dom (path)
#                    → muskitty-selectors (path) → muskitty-css (+ parser + tokenizer)
#                                                 → muskitty-dom (path)
#
# All path deps are independent repos under muskitty-dev/.
#
# Auth: GitHub sometimes rate-limits anonymous git clones from CI runners,
# returning 401 and prompting for a username (which fails in non-interactive
# shells). When GH_TOKEN is provided via env, rewrite https://github.com/ URLs
# to use x-access-token auth. This works for any public repo the token can read.
#
# Idempotent: skips clones that already exist (useful for local re-runs).
set -euo pipefail

if [ -n "${GH_TOKEN:-}" ]; then
    git config --global "url.https://x-access-token:${GH_TOKEN}@github.com/".insteadOf "https://github.com/"
fi

clone_if_absent() {
    local url="$1"
    local dest="$2"
    if [ -d "$dest" ]; then
        echo "skip $dest (exists)"
    else
        git clone --depth 1 "$url" "$dest"
    fi
}

clone_if_absent https://github.com/muskitty-dev/muskitty-css.git ../muskitty-css
clone_if_absent https://github.com/muskitty-dev/muskitty-css-parser.git ../muskitty-css-parser
clone_if_absent https://github.com/muskitty-dev/muskitty-css-tokenizer.git ../muskitty-css-tokenizer
clone_if_absent https://github.com/muskitty-dev/muskitty-cssom.git ../muskitty-cssom
clone_if_absent https://github.com/muskitty-dev/muskitty-dom.git ../muskitty-dom
clone_if_absent https://github.com/muskitty-dev/muskitty-selectors.git ../muskitty-selectors
