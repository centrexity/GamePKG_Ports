# Zelda3 for GamePKG

Engine: https://github.com/snesrev/zelda3
Pinned revision: 45a149d8b4bb8098f53d1009f76e76e7d8b79bbf.
MIT code copyright 2022 snesrev and 2021 elzo_d; complete notices are included.
GamePKG adaptations are Apache-2.0. Original Zelda game data belongs to its
respective owners and is not distributed with this package.

The restricted Rust entry calls a host-owned, pinned engine compiled into Wasm
under isolation. The engine has no filesystem/network imports. The selected
USA ROM is decoded into temporary RAM resources without extracted asset files.
Game SRAM uses the creator-scoped save API. Optional host-controlled snapshot
slots use separate private user storage; game source receives no file paths.

Arrows/WASD move; Z sword/B; X equipped item/Y; C interact/A; V map/X;
Enter Start/pause; Tab Select/save menu; Q/E L/R.

Thumbnail artwork credits, source links, modifications and license terms are included in NOTICE.

Current host builds also check the complete reviewed source-kit identity in
cached Wasm engines, alongside the renderer revision and normal sandbox/build
validation. Older caches require rebuilding; Integrity fix is verified against a
fully callable engine with only its identity section removed. This host metadata
contains no ROM resources and does not change the signed package or SRAM format.

The reviewed save-state DMA component uses explicit little-endian fields and
canonical padding instead of native struct extent. Its Apache-2.0 adaptation
preserves the upstream source file and original 64-bit layout. Sanitized and
actual 32-bit component checks pass; full replay/save-state tools remain separate
work and existing SRAM behavior is unchanged.

The corresponding Apache-2.0 DSP component codec preserves the original
3024-byte state layout with explicit little-endian words, canonical padding and
validated playback fields. It retains the live audio-RAM pointer. Sanitized and
actual 32-bit checks include exact original stereo sample-buffer output;
whole-state replay/save tools remain unfinished.

The PPU component adaptation uses explicit little-endian video/palette/tilemap
fields and validates layer flags while preserving the original component's
field boundaries and live rendering state. Its Apache-2.0 codec is covered by
sanitized and actual 32-bit callbacks; full snapshot/replay integration remains
separate work.

An internal engine snapshot bridge now uses the original save/load callbacks
with bounded version/profile/checksum framing and component preflight. Synthetic
host/32-bit callbacks and a private ROM-backed Wasm title/gameplay/inventory
round trip are verified, including exact subsequent pixels and stereo samples.
The host now connects snapshot transfer to native and desktop Wasm sessions,
including creator/game ownership, frame-counter restoration and reservations
within the existing memory allowance. The audio restore adaptation recovers
the omitted echo-delay cache from the saved DSP register, with sanitized and
actual 32-bit register-trace checks and private native/Wasm playback proofs.
Original Shift+F1–F10 save/F1–F10 load bindings now control persistent private
snapshot slots, with all 20 configured slots available to headless CLI runs.
Creator/game/schema-scoped files use checked framing and directory-relative
atomic replacement. Replay recording, legacy snapshot imports and complete
untrusted game-state validation remain separate work. No ROM resources or
snapshot recordings are included in the package.

The Apache-2.0 SPC map adaptation explicitly encodes native 16-bit audio-state
words as little-endian bytes while preserving port arrays and unmapped live
state. Its production copy functions match the original on 32/64-bit
little-endian builds and a fixed byte digest on emulated big-endian PowerPC.
Original upstream files remain unchanged. This component proof does not claim
complete big-endian gameplay or hardware qualification.

Live SPC reads now use explicit unaligned-safe little-endian access and audio
RAM address wrapping; high-byte operations are numeric. The reviewed DSP patch
also removes undefined negative left shifts during sample decoding. Strict
production-player tests cover synthetic uploads and audible stereo songs with
identical PCM/state digests at both little-endian pointer widths and emulated
big-endian PowerPC. Existing upstream tremolo/new-music stubs and broader
audio/gameplay qualification remain open. No music recordings are packaged.
