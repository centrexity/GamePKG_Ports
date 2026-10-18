# Arkanoo for GamePKG

Adapted from c0m4r/arkanoo: https://github.com/c0m4r/arkanoo
Upstream revision: 9d613e78c9ed5657ae7fda9206cd4a82a893b7c8
Copyright (C) 2025 c0m4r. Upstream code: WTFPL version 2.
Upstream credits: c0m4r, Gemini 3 Pro, Claude Sonnet/Opus 4.5.

This port adapts nine campaign and twelve procedural layout rules from entities.rs,
paddle/ball motion and spin, overlap collision response, scoring, and the
long-paddle, extra-ball, ghost-ball and rocket mechanics from entities.rs and
game.rs. The engine integration has been rewritten for GamePKG's safe APIs.

Most rendering, bitmap lettering and synthesized audio are new; original shard
particle behavior/drawing is restored as detailed below. Upstream Pixabay
music/sound, fonts, logos and generated background art are not redistributed.
The port uses a 640x480 canvas and GamePKG host randomness instead of SDL2,
rand or filesystem access. Launch directions use rejection sampling, and bonus
selection uses independent probability/weight draws.
Endless levels use the original twelve procedural patterns plus saved custom
patterns. The editor supports painting/erasing, all nine brick choices, names,
save/load, clear confirmation and test play through scoped host APIs. Remaining menu/music behavior, penguin/heart-loss animation still needs
fidelity work. Original ASCII pattern files can now be imported/exported through
player-selected files.

Controls: mouse or Left/Right/A/D move; Space launches attached balls or fires
a rocket. Escape pauses/resumes; R restarts from pause/game-over/victory; Q skips
a playing level. Up/W and Down/S retain their controller-friendly aliases;
Enter confirms start/next level, and V/controller X toggles gravity. E opens the
editor from the title/pause screen. In the editor, 0–8 selects a brick, left drag
paints and right drag erases. N edits the name, S saves, C twice clears, L browses
saved patterns, Up/Down and Enter loads, T tests and Escape exits. Test completion
or game over returns to the unchanged editor board. Three lives, nine campaign
levels, then endless play. High score and unlocked level persist in the game's
scoped save. Active play state stays inside this game's isolated process or Wasm
instance and is not yet saved across launches.

Thumbnail artwork credits, source links, modifications and license terms are included in NOTICE.

Gravity uses the original downward-only acceleration (0.3 original units/frame),
terminal speed 9, spin suppression in gravity mode and 0.98 spin decay otherwise.
Velocity/spin are represented at half scale in this port. Six trajectories match
a compiled, unchanged fragment of the pinned original source. Paddle acceleration
no longer clamps the ball below the original portal threshold.

At 3600 original pixels/second, the portal activates once per level and awards
5000 points. Balls orbit at radius 150 original pixels and 0.1 radians/frame;
blocks move inward at 8 original pixels/frame with original integer rounding.
After absorption, the 270-frame completion delay precedes level transition.
New levels clear portal state. Portal animations and absorption freeze on pause.
Native and Wasm fixtures exercise the production physics and renderer directly.
Legacy ARK1 gravity used its reserved word; ARK3 retains that metadata through
its packed flags and reads both older formats. Full run saving and
the broader physics/geometry differences above remain unfinished.

Ball-to-ball contacts now restore the original strict distance test, upward
minimum-speed ejection and horizontal separation, including coincident balls,
simultaneous pairs and portal exclusion. Seven fixtures execute the unchanged
contact block from the pinned original and match the port's half-scale velocities
and 36-particle counts. The cyan shockwave is rendered by the real native/Wasm
fixtures. Original shard lifetimes, shapes and fading are now restored as
detailed below; this does not establish complete visual/physics parity.

World coordinates now preserve the original 1280x720 proportions at uniform
half scale, letterboxed at y=60: balls have radius 3, bricks are 30x10, the
paddle is 70x11 at y=395 and moves 9 units/frame. The long paddle is 90 units
wide, keeps its center on activation/expiry and retains the original velocity
history. Both-direction input applies original per-direction clamping.
Unattached balls use the original single movement step, wall clamps, downward
loss boundary and minimum horizontal velocity. Contacts run after movement in
the original order: ball pairs, paddle, blocks, rockets, bonuses, portal,
particles and lost-ball cleanup. Ghost balls can hit every intersected block;
normal balls stop after the first block. Collision rectangles reproduce SDL's
original integer truncation before intersection. Compiled unchanged ball and
paddle code plus actual SDL intersections supply independent regression data.
The original brick-resolution fragment also verifies all four sides, corner
ties, fractional positions, misses and ghost pass-through.
Native/Wasm tests verify brick resolution and pair-before-paddle ordering;
desktop/browser checks verify playable controls and the projected geometry.
Visual effects and other remaining features above still need their separate
fidelity work.

Every ball now starts stationary with its own 30-frame countdown. Automatic
launch occurs on the following update and moves on the next frame; manual
launch moves immediately. All three original initial directions and vertical
speed are retained. Input launches attached balls before considering rocket
fire, so launching does not spend ammo. Extra balls start attached using the
original spawn offset. The old four-ball, nine-ammo, four-rocket, eight-drop,
9,999,999-score and level-999 restrictions are removed; portable integer/time/
memory bounds still apply. Number rendering and legacy ARK1 saves preserve
larger scores and levels.
Losing a life deducts up to 20 points, preserves the paddle, bonus timers,
ammo, drops and rockets, and creates a fresh attached ball if lives remain.
A cleared level restores one life when any life was lost on that level, up to
three. Level nine has a distinct victory screen before endless mode. Starting
a new level resets paddle/items/portal state, while game-over restart resets
the run and retains persisted best/unlocked metadata. Compiled unchanged
original launch/update/loss/completion fragments verify these logic paths;
native and Wasm checks also cover save compatibility, launch frame timing,
bonus preservation, victory and progression through level 1000. Heart-loss
animation and original menu presentation remain separate unfinished work.

Brick logic now includes normal, two-hit ice, explosive and metal types. Metal
resists ball hits and is ignored when deciding whether a level is clear; portals
still consume it. Ice keeps damage between hits and shows cracks. Explosive
ball hits queue a 60-original-pixel radius blast; rockets use 120 original pixels.
Both use actual block centers, including blocks moved by a portal. Indirect
blast damage bypasses ice health and metal armor, never rolls a bonus, and does
not chain through explosive bricks. Only directly destroyed ball-hit bricks roll
bonuses. Repeated rockets and ghost-mode contacts retain original ordering.
Ten fixtures execute the unchanged original brick and rocket phases against
actual SDL intersections. Native/Wasm tests verify ice damage and type rendering,
world-distance boundaries/no chaining, and a complete 200-special-brick screen
within normal time, image and drawing budgets. The added ice/magma/metal images
retain the original base colors and damage cues but do not reproduce every
upstream gradient, refraction or animated fire/shine effect.
The pinned upstream generator deliberately disables special bricks in ordinary
procedural campaign/endless layouts; custom patterns can contain them. The
validated type-setting logic is now used by the editor and its test game.
Original ASCII pattern-file import/export is now mediated by the host file picker.

Procedural generation matches the pinned original `create_blocks` with no custom
patterns installed: the level/attempt seed formula, twelve pattern choices,
per-cell and mirror seeding, parameter ranges and the 20-brick/10-retry rule.
The public seeded stream is a scalar ChaCha12/PCG and rand 0.8.5 sampler
adaptation; it adds no guest crates and never consumes gameplay entropy.
Explicit fixed-width sampling preserves the pinned 64-bit layouts on both native
and wasm32, avoiding the original `usize`-dependent selection. Rand attribution
and MIT terms are in NOTICE and assets/LICENSE-RAND.txt. The complete unchanged
original generator and pinned original Rand dependency supply 106 independent
layout fixtures, including all twelve types, five low-density retries and
levels through u32::MAX. Raw streams and integer/float/Bernoulli distributions
also match independently. Actual native and wasm32 runtime fixtures compare
all 106 rendered boards, including brick coordinates. The custom-pattern pool is
now verified separately against the unchanged original generator and converter.

The port now consumes focused, ordered GamePKG input events. Mouse coordinates
are projected into the original world and truncated to original integer paddle
positions, preserving motion-derived spin. Space launches/fires in event order;
Escape toggles pause, and transition/victory screens accept a left click.
The browser Restart action also resets the active run instead of retaining the
old static state. Native/core/component SDK probes verify pointer positions,
physical key repeat bits, committed Unicode and denied input permissions.
Actual native-window and both browser profiles verify scaled mouse movement
and Escape pause; runtime fixtures verify Space and paused mouse behavior.
Gameplay now requests hidden, confined absolute input, while pause and the
editor release it. F11 toggles the host's actual fullscreen state; Q quits from
pause/game-over/victory while retaining its original play cheat. These requests
use the bounded GamePKG window API and commit only after a successful tick.
Focus loss, file selection and emergency release restore the host cursor.
Native window tests verify actual fullscreen, visibility and clean quitting;
core/component/native runtime tests verify bindings and repeat rejection.
The native host now remembers normal window size, fullscreen and maximized state
per game/creator/library. Linux window tests verify resize and combined-mode
reopen, original normal-size restoration, game/OS close and damaged-record
preservation. Original splash artwork, remaining menu presentation, resolution/VSync settings UI,
initial modifier state and full platform coverage remain unfinished. This does not claim complete
original input/window parity. The updated source host is required.


The editor adapts the pinned `editor.rs` and preserves its integer grid bounds,
paint-without-replacement behavior, left-drag priority when both buttons are
held, all nine color/type choices, clear confirmation, sorted saved-pattern
browser, name editing and test return. Newly painted ice has two hits; loading
an editor pattern gives blocks one hit and a matching maximum health, preserving
its undamaged appearance just as the original loader does. Patterns
selected by the endless generator instead use the original `Block::new` health.
The original six normal-brick RGB colors are restored in play and the palette.
Background selection currently changes replacement colors; original background
art and fonts remain absent. Unicode names persist but the bitmap font displays
non-ASCII characters as question marks. Native IME activation still needs work.

`ARK3` saves contain high score/unlocked level/gravity, effects volume/mute and
named patterns in the same 1 MiB creator/game-scoped blob. The header stays 20
bytes: little-endian flags at offset 12 use bit 0 for gravity, bits 1–8 for volume
0–128, and bit 9 for mute. Other bits and out-of-range levels are rejected. Pattern
encoding/capacity stays unchanged. Valid `ARK1` and `ARK2` saves load with the
original effects default (64, unmuted), then migrate on the next successful save.
Zero volume is distinct from a legacy default. Older recipes preserve `ARK3` as
unrecognized data but cannot load it.
Oversized, malformed and future data remains preserved. Denied/failed saves are
shown in the editor and effects settings; failed editor writes roll back the saved
pattern collection while retaining the working board. Names are identifiers,
never host paths. Active runs and unsaved editor changes still do not persist.
The original ASCII reader/writer is retained and compared with original files;
external import/export now uses the signed, host-mediated file workflow described below.

`tests/reference/arkanoo_editor.py` compiles the complete unchanged original
editor module with filesystem operations confined to a temporary maintainer
folder. Its fixtures verify editing, duplicate placement, erasing, boundaries,
loaded health, ASCII compatibility, clear confirmation, Unicode backspace and
background cycling. Rendering/button hover is outside that reference fixture.
`arkanoo_levels.py --custom` compares 106 layouts with the original generator
and pattern-to-block converter using a three-pattern pool, including special
bricks and an empty pattern. Production uses the saved browser's sorted order
for the pool; upstream directory enumeration order is filesystem-dependent.
Actual native and wasm32 fixtures cover all 106 custom-pool boards, editor
save/load/test return, denied saving and damaged-save preservation. Native-window
and both browser profiles also exercise editor painting and scoped persistence.


I imports an original ASCII `.txt` pattern; X exports the current board. Native
playback uses explicit system Open/Save dialogs. Browser playback uses a file
upload control and an explicit Save/download action. Cancellation, invalid UTF-8,
denied file permissions and unavailable headless dialogs preserve the working
board. Imported files stay unchanged. S commits an imported/edited pattern to
the game-scoped collection; exporting does not change the save. Existing ASCII
comments, short rows, empty cells and legacy I/E/U symbols retain the original
parser behavior. Names are derived from the selected basename, without exposing
the host path. Filename suggestions are sanitized and remain editable in Save.
Both imported text and save archives use bounded 64 KiB reads; archives larger
than one chunk now reload correctly. The new file API requires an updated source
host and is not a grant of general filesystem access or Cargo dependencies.

`tests/arkanoo.py` verifies native system-file-dialog cancellation/import/export
and both browser profiles' actual file upload/download actions. The actual
native/core/component runtime tests also cover invalid UTF-8, omitted file
permissions, headless denial, unchanged source files and exports to a player
chosen destination. A play-history write failure cannot expose a new dialog.
Archives beyond 64 KiB reload through chunked reads, while damaged data stays
preserved. Scoped directory browsing and multi-file resource workflows remain
separate host requirements; this is single-file pattern selection.

The main menu now exposes New Game/Resume, Restart, Gravity Mode, Level Editor,
Settings and Quit. New Game starts the initialized run; Resume preserves the
paused ball and board. Restart resets score, lives, items and the active run while
retaining saved patterns/preferences. Primary mouse
targets use the original 1280x720 rectangles
projected into the existing canvas; hover colors and borders follow the original
menu. A fresh launch starts level one, Resume keeps the paused board, and Restart
retains the port's saved metadata/preferences and passes through the original
splash/paused-menu flow described below.
Legacy Up/W quick-start still uses the highest unlocked level. Settings now
provides fullscreen/back and effects-volume/mute controls; it is not the complete original page.
Music settings, resolution,
VSync/confirm/revert controls, project-homepage button, fonts and background art
still need restoration. No inactive placeholder controls are presented as working.

`tests/reference/arkanoo_menu.py` compiles the complete unchanged original menu
module, guarded by its pinned source hash. Its 81 button-boundary cases and 25 slider-event observations
supply `tests/fixtures/arkanoo-menu.json`; unsupported original menu features
remain explicitly listed. Unit and actual native/core/component runtime checks
exercise actions and pause preservation. Linux native and both browser profiles
click Settings, fullscreen/back and New Game before their normal gameplay/editor
and file-dialog tests. Fullscreen browser image scaling now preserves proportions
and translates pointer coordinates through borders and aspect-ratio bars.

Effects settings retain the original 0–128 range, default 64, mute independence,
integer drag quantization and endpoint clamping. Pointer motion starts a drag
inside the track; release/reset cancels it. New Game/Restart retain the selected
values. Percent fill/text follow original integer rounding. Changes adjust the
current synthesized effects; original MP3 effects and music are still absent.
The effects phase is now reduced before adding sample indices, removing the old
32-bit `usize` overflow at large frame values. Actual native and wasm32 samples
at `u32::MAX` match an independent widened reference.

Unit checks cover every volume/mute encoding, legacy/full-capacity archives,
reserved flags, invalid levels, truncation and unchanged data after decode failure.
Actual native/core/component tests verify measured PCM at quarter/default/full
volume, mute/zero silence and reopen, legacy defaults/migration, and warning/data
preservation when saving is denied or damaged. Linux desktop and both browser
profiles use actual drags and mute clicks; browser Restart restores the rendered
settings as well as the stored flags. These checks do not close original recorded
audio, music, complete menus or non-Linux gameplay requirements.

Startup now uses a separate splash state for the original 270 update frames.
Enter and left click skip it without also starting gameplay. Escape, right click,
editor/quit shortcuts do not dismiss it; F11 remains available. The timer lives
across run resets, as in the original main loop: an expired splash immediately
returns to the menu after Restart, while an early skip leaves its remaining
frames available on a later restart. The menu retains whether play has started,
so editor return and restart preserve the appropriate New Game/Resume action.
Returning from the editor no longer silently discards the paused run when Resume
is clicked. GamePKG retains saved patterns and effects/gravity preferences across
run resets; active runs still do not survive closing the application.

Seventeen independent state observations compile unchanged pinned constructor,
reset, pause and main-loop splash/skip/editor-exit fragments. Source hashes guard
both complete upstream files. SDL window calls and entity allocation are stubbed
only for these state tests. Original splash artwork/audio remain absent: the
current title screen is replacement art, not visual parity.

Original shard particles now use 20–39 update lifetimes, sizes 3–7, random rotation
and spin, 0.3 original-pixel gravity, linear fading and integer-rotated diamond
points. Coordinates remain in the original 1280×720 space until rendering to
avoid accumulated scale/letterbox rounding. Original point order and repeated
points are retained. Broken bricks queue their original-color 10–15 shards until
after rocket processing. Manual launch creates 20 gold shards per attached ball;
speed records create five icy shards, portal activation 100 purple shards, and
absorbed bricks five purple shards. The shockwave retains its 36 radial shards.
There is no replacement 64-particle cap.

A clipped RGBA layer batches all shards into at most six 512×512 image tiles
rather than silently losing effects at the host command or texture-size limits. Independent fields use disjoint
bits of host random words with rejection sampling for unbiased lifetime/size
ranges. This preserves random distributions without exceeding native host-call
limits on full-board bursts. No dependencies, macros or indirect calls were added.
The host safety budgets remain unchanged.

`tests/reference/arkanoo_particles.py` compiles the complete unchanged original
Particle implementation and draw function, with full-file SHA-256 guards. Its
114 observations cover age/death, position/velocity/rotation, fading, negative and
edge coordinates and the exact ordered shard points. SDL drawing calls are
recorded rather than replaced with a second drawing algorithm. Native and wasm32
fixtures inspect the actual image coordinates/pixels and exercise 2,000–3,000
shards from one full-board burst. A full-viewport stress case verifies all four
corner shards survive tiling under the unchanged memory/image limits. Penguin/
heart loss, original ball artwork/speed lettering and remaining music/audio are
still separate open work.

Ball trails now track original top-left positions before force/movement updates
at 800/1400 original px/s. The original retains eight or twenty positions by
popping a single entry after each insertion; dropping from the higher tier
therefore retains its longer history until speed drops below 800. Attached balls
and portal-orbit updates preserve existing history; a new ball starts empty.

Regular/super fireball effects restore 20/30 trail samples, original integer
circle points/radii, color gradients and alpha, super-fireball pulse, and twelve
spinning glowing particles. Effects draw before their own ball through the same
bounded tiled RGBA layer used by shards, preserving per-ball order and clipping.
No new host capability, crate, macro, indirect call or resource-limit increase is
required. Original metal ball artwork and speed-text font remain separate work.

`tests/reference/arkanoo_trails.py` compiles the unchanged original Ball update
and effect-drawing fragments with full-file SHA-256 guards. Forty-nine history
observations cover threshold crossings, tier downgrade, pre-force timing and
attached-state freezing. Sixteen effect cases record all 15,909 point/color
operations at ordinary/threshold/high speeds, negative/edge positions, wide
history and large frame values. Source tests match exact point order/colors;
actual runtime tests inspect the resulting RGBA images and tiled coordinates.

The original u64 animation clock is preserved rather than resetting at 32-bit
wraparound. Original-code effect cases include u32::MAX, the following frame,
and u64::MAX. Signed runtime probes verify the stored clock crosses the 32-bit
boundary and remains paused when requested. Synthesized effect phase now also
uses u64 modulo reduction before adding its bounded sample index; independent
u128 vectors and actual native/wasm32 PCM verify both integer-limit boundaries.
