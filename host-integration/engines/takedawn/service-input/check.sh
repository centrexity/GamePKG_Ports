#!/bin/sh
# Maintainer component check. Bindings come from the app, never the package.
set -eu
app=${1:-/media/user/660p1/c-site1/GamePKG/app}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
scratch=$(mktemp -d "${TMPDIR:-/tmp}/takedawn-service-input.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
rustc --edition=2021 -D warnings --crate-name gamepkg --crate-type rlib \
    "$app/engines/native-services/client.rs" -o "$scratch/libgamepkg.rlib"
rustc --edition=2021 -D warnings --test "$here/tests.rs" \
    --extern "gamepkg=$scratch/libgamepkg.rlib" -o "$scratch/pad-tests"
"$scratch/pad-tests"
# Compile the non-test path too, against the same unmodified supplied client.
rustc --edition=2021 -D warnings --crate-type lib "$here/pad.rs" \
    --extern "gamepkg=$scratch/libgamepkg.rlib" -o "$scratch/libpad.rlib"
