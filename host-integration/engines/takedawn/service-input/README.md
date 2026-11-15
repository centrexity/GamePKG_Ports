# Burnout controller service adapter

`pad.rs` is dependency-free game source. It translates the host's four logical
players into Duke/XInput reports and sends bounded rumble through GamePKG. It
does not enumerate devices or call SDL, XInput, Windows, or Linux directly.
It is a migration component; the full Takedawn engine does not yet consume it.

Only `pad.rs` belongs in a game source kit. `tests.rs`, `source_check.rs`, and
the scripts are maintainer tools. Test attributes, derives, and assertions must
not be copied into the game package. Bindings are supplied by the app after
source admission; they must not be bundled as game source.

Run the value and lifecycle tests with:

```sh
sh check.sh /path/to/GamePKG/app
```

For the complete component check, first build the app library, then pass its
exact `libgamepkg-*.rlib` path:

```sh
python3 check_source.py --app /path/to/GamePKG/app --app-lib /path/to/debug/deps/libgamepkg-HASH.rlib
```

The latter also signs a temporary synthetic package, checks the exact production
adapter before injecting bindings, and compiles optimized audited game objects
for Linux and Windows at both CPU widths. It requires the app's pinned target
toolchains and Linux preparation confinement support. The public test seed is
not the ports creator credential. Unused closures, attributes/macros, aliases,
and filesystem calls must still fail admission.

Validation on 2026-11-14: four unit tests passed, including exhaustive 16-bit
button masks and analog ranges, packet changes, focus loss, disconnect/recovery,
and logical-player errors. All four target builds and four rejection cases
passed against app commit `7d350e10572cf61e451f623324d78858fd327784`.
These are synthetic/component results, not physical controller, rumble,
gameplay, audible audio, or frame-pacing qualification.
