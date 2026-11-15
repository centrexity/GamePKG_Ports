#!/bin/sh
# Synthetic maintainer checks. Python extracts public source; players need none.
set -eu
app=${1:-/media/user/660p1/c-site1/GamePKG/app}
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../../../.." && pwd)
scratch=$(mktemp -d "${TMPDIR:-/tmp}/takedawn-nvi1.XXXXXX")
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
python3 "$here/review_contract.py"
tar -xzf "$root/takedawn/assets/takedawn-source-kit.tar.gz" -C "$scratch" \
    toolkit/src/d3d/d3d8_vsh_parse.c toolkit/src/d3d/d3d8_vsh_parse.h \
    toolkit/src/d3d/d3d8_combiners_parse.c toolkit/src/d3d/d3d8_combiners_parse.h \
    toolkit/src/kernel/nv2a_vsh_fields.h toolkit/src/kernel/nv2a_combiner.h toolkit/src/kernel/nv2a_combiner.c
cc -std=c11 -O2 -Wall -Wextra -Werror -I"$scratch/toolkit/src/d3d" -I"$scratch/toolkit/src/kernel" \
    "$here/oracle.c" "$scratch/toolkit/src/d3d/d3d8_vsh_parse.c" \
    "$scratch/toolkit/src/d3d/d3d8_combiners_parse.c" "$scratch/toolkit/src/kernel/nv2a_combiner.c" -lm -o "$scratch/oracle"
rustc --edition=2021 -D warnings --crate-type rlib --crate-name takedawn_nvi1 \
    "$here/nvi1.rs" -o "$scratch/libtakedawn_nvi1.rlib"
rustc --edition=2021 -D warnings --crate-type rlib --crate-name host_nvi1 \
    "$app/src/shaders/nv2a.rs" -o "$scratch/libhost_nvi1.rlib"
rustc --edition=2021 -D warnings --test "$here/tests.rs" \
    --extern "takedawn_nvi1=$scratch/libtakedawn_nvi1.rlib" \
    --extern "host_nvi1=$scratch/libhost_nvi1.rlib" -o "$scratch/tests"
TAKEDAWN_NVI1_ORACLE="$scratch/oracle" "$scratch/tests"
