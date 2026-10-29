# Credits

DOOM by id Software. Engine portability work by Chocolate Doom and doomgeneric contributors; see upstream source headers for individual notices.

GamePKG integration by GamePKG contributors. The engine and adapter retain GPL-2.0-or-later licensing. The package recipe calls only the host-injected API and has no Cargo dependencies.

Thumbnail artwork credits, source links, modifications and license terms are included in NOTICE.

Host input integration follows SDL's X11 button numbering for extra mouse buttons.
The sixth physical button uses the original engine's eighth mouse-state bit,
separate from vertical wheel pulses. The maintainer SDL/XTest reference and
native/Wasm configured-binding tests cover this mapping; other desktop input
backends remain separate qualification work. See the upstream
[SDL X11 event handler](https://github.com/libsdl-org/SDL/blob/SDL2/src/video/x11/SDL_x11events.c).

The host's scoped configuration reader validates saved key/button values before
calling the pinned original parser. Unsafe indexes and malformed values are
ignored without skipping later settings; valid disabled bindings, hexadecimal
forms, comparison-only values and the original joystick autorun setting remain.
Sanitized and actual 32-bit storage checks plus original native/Wasm gameplay
fixtures cover these paths. Other configuration fields remain separate work.

The reviewed adapter restores the original gamma variable's saved-config binding,
so F11 gamma changes survive relaunch. Documented gamma, game-view size and sound
volume ranges are checked before original rendering/audio use; the low-detail
setting keeps its documented zero/nonzero meaning. Native/Wasm tests cover
recovery, maximum valid settings with visible frames/audible music and F11
persistence. Sanitized and real 32-bit storage cases cover the numeric reader.
The original upstream implementation remains unchanged.

Saved mouse sensitivity retains the original extended range of 0–30. Invalid
values fall back to the engine default rather than reaching the original Options
thermometer, whose configuration documentation warns that 31 or higher crashes.
Native/Wasm tests reload a damaged value, draw the Options menu at 30 and reduce
it through the original controls; sanitized and actual 32-bit reader tests cover
all accepted values, boundaries, malformed numbers and following-setting recovery.

The saved effect-channel count is checked against the adapter's existing
16-channel mixer capacity before original allocation. Zero still disables effect
channels without disabling music. Invalid values restore the engine default of
eight channels. Native/Wasm reload tests cover invalid, 16-channel and zero-channel
settings; these checks do not independently qualify 16 simultaneous active effects.

The separate production-mixer fixture verifies all 16 voices with staggered
48/24 kHz impulses, pinned original stereo levels, independent completion and
silence after completion. Sixteen simultaneous opposite-polarity voices cancel
before clipping; disabling either final voice exposes its expected signed PCM.
These asset-free checks run with sanitizers and as actual 32-bit code. They
qualify the mixer primitive separately from in-game channel allocation/stealing.

A separate fixture compiles the unchanged original sound-channel allocator and
stop routines directly. With observable device callbacks, it checks allocation,
priority rejection/stealing, same-source replacement, null origins, shared-effect
bookkeeping and already-completed voices for every channel count from zero to 16.
Sanitized host and actual 32-bit runs pass. This isolates original allocation
semantics; complete in-game allocator-to-mixer and hardware behavior remains
separate qualification work.

The reviewed build adaptation corrects the original sound-ID upper-bound check:
the table length is rejected before indexing, alongside zero and negative IDs.
The pinned upstream source remains unchanged. Sanitized and actual 32-bit dispatch
fixtures reject invalid boundary IDs and accept all 108 valid original IDs;
native/Wasm gameplay and audio regressions cover the rebuilt engines.

The sound-distance adaptation widens coordinate differences and approximate
distance before the original clipping/attenuation rules. It avoids signed
overflow and preserves ordinary distances and the original map-8 distant-sound
rule. Synthetic sanitizer and actual 32-bit fixtures cover 18,626 cases, with a
controlled angle callback; renderer coordinate-to-angle arithmetic and complete
spatial playback remain separate qualification work.

The reviewed point-to-angle adaptation widens coordinate differences and their
magnitudes before the original octant/slope lookup. It preserves the original
unsigned slope arithmetic and exact ordinary outputs. Original defined-input
and corrected outputs agree across 143,042 cases; 250,625 boundary/random cases
have identical pinned 32/64-bit digests. An additional 18,626 distance cases run
with the actual corrected angle backend at both widths. Original-asset spatial
PCM, other renderer geometry and hardware output remain separate work.

Music selection now honors the backend's advertised original device list:
ADLIB (2) and Sound Blaster (3) use the current OPL backend; NONE (0) and
unsupported devices do not initialize it. Unsupported MIDI/GUS backends are
not substituted silently. Before initialization and after shutdown the bounded
music renderer returns silence. Sanitized/actual 32-bit lifecycle and selector
tests, all 45 selected-WAD song startup checks and native/Wasm saved-device
relaunch checks pass. Additional music backends remain separate work.
