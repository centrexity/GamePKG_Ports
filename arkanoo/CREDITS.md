# Arkanoo for GamePKG

Adapted from c0m4r/arkanoo: https://github.com/c0m4r/arkanoo
Upstream revision: 9d613e78c9ed5657ae7fda9206cd4a82a893b7c8
Copyright (C) 2025 c0m4r. Upstream code: WTFPL version 2.
Upstream credits: c0m4r, Gemini 3 Pro, Claude Sonnet/Opus 4.5.

This port adapts nine campaign and twelve procedural layout rules from entities.rs,
paddle/ball motion and spin, overlap collision response, scoring, and the
long-paddle, extra-ball, ghost-ball and rocket mechanics from entities.rs and
game.rs. The engine integration has been rewritten for GamePKG's safe APIs.

The safe host integration and bounded PCM mixer are new. Original visual
procedures/assets restored in the port are identified below, including brick,
ball and paddle art, backgrounds, rasterized Roboto lettering and startup art.
All five original recorded sound effects are restored with the provenance below. The canonical source also contains the original music controller, music settings and shared host effect playback. Complete recorded-audio assembly remains private while asset delivery is unfinished; the default package retains its effect-only profile. Individual artist/item credits for bounce, glass and launch were not supplied by upstream.
The port uses a 640x480 canvas and GamePKG host randomness instead of SDL2,
rand or filesystem access. Launch directions use rejection sampling, and bonus
selection uses independent probability/weight draws.
Endless levels use the original twelve procedural patterns plus saved custom
patterns. The editor supports painting/erasing, all nine brick choices, names,
save/load, clear confirmation and test play through scoped host APIs. Remaining menu/music behavior still needs fidelity work. Original ASCII pattern files can now be imported/exported through
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
animation is restored below; original menu presentation remains unfinished.

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
within normal time, image and drawing budgets. Later rendering changes below restore the normal/graphite textures and ice
refraction, glints and moving shine. The explosive fire/glass restoration is described below; full-frame
compositing fidelity remains separate unfinished work.
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
corner shards survive tiling under the unchanged memory/image limits. Original ball artwork/speed lettering and remaining music/audio are
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


Penguin/heart-loss behavior now uses the unchanged original 5.5-pixel approach,
30-frame grab and 7/5-pixel escape, with original ordered rectangle geometry,
hover/flicker and held-heart positions. The original final-life GameOver freeze
is retained. Independent SHA-guarded original-code fixtures cover 337 animation
frames, sixteen static drawing poses and three fallback-heart shapes. Actual
native/wasm32 checks compare state, RGBA rectangles and HUD texture positions.
Pause, editor, repeated loss and reset behavior have regressions.

`assets/heart.png` is copied unmodified from c0m4r/arkanoo at the pinned revision
above, under its WTFPL version 2 license (see LICENSE):
https://github.com/c0m4r/arkanoo/blob/9d613e78c9ed5657ae7fda9206cd4a82a893b7c8/assets/heart.png
SHA-256: `86b8ef21b5544f39542c247518155f4a856aab70851bbd76cfb67df4bd0fbd71`.
It uses the original linear texture filtering through GamePKG's bounded asset
helper. This source version requires the current source-built GamePKG host;
released 0.1.17 binaries lack that helper. No guest dependencies or additional
file permissions are introduced. Numeric HUD text remains a replacement;
restoring this heart asset does not establish full original HUD/art parity.

The metallic ball texture is now restored from the unchanged
`draw_shiny_ball_texture` procedure in the pinned upstream `src/rendering.rs`.
`assets/ball.png` contains its 12×12 RGBA result, including SDL target clipping,
and inherits the upstream WTFPL-2.0 terms in `assets/LICENSE-ARKANOO.txt`.
The reference generator is `tests/reference/arkanoo_ball.py` in GamePKG; its
fixture records source/fragment and decoded-pixel hashes. The port draws it with
linear filtering at the original integer rectangle coordinates, at half world
scale. The original speed label and remaining presentation are still unfinished.

The rounded glass/metallic normal and extended paddle textures are restored from
unchanged `draw_paddle_texture` code in the same pinned source. `assets/paddle.png`
(140×22) and `assets/paddle-long.png` (180×22) inherit WTFPL-2.0. The maintainer
reference generator `tests/reference/arkanoo_paddle.py` uses real SDL software
rasterization for clipping and alpha blending, and records the unchanged
`draw_paddle_with_glass` overlay rectangles at normal, extended and spin states.
The port restores the cyan core, orange end lights, collision-triggered purple
and white discharge, aura, decay and original integer positioning. Ghost-ball
bonuses do not recolor the paddle, matching upstream. Other art, recorded audio,
fonts and menu/settings presentation remain unfinished.

The six normal-brick textures (`assets/brick-red.png`, `brick-orange.png`,
`brick-yellow.png`, `brick-green.png`, `brick-blue.png`, `brick-violet.png`) are
generated from the unchanged pinned `draw_block_texture` procedure and original
`BLOCK_COLORS`. They preserve the glass/metallic gradient, inner glow, SDL line
clipping and beveled borders under WTFPL-2.0. GamePKG's maintainer reference
`tests/reference/arkanoo_blocks.py` compiles that procedure and rasterizes its
commands through real SDL; every decoded pixel is checked. Gameplay and placed
editor bricks share the filtered textures and original integer coordinates.
The original palette remains flat colors. Later sections cover explosive
glass/fire layers; full frame compositing equivalence remains unfinished;
this does not establish complete brick, art or presentation parity.

The graphite brick now uses `metal-body.png`, `metal-stripes.png` and
`metal-bevel.png`, generated from the unchanged original Undestroyable branch of
`draw_block_with_gradient`. The opaque 60×20 body matches every original SDL
pixel, including rivets and bevels. Two separate 91×21 nearest-sampled layers
retain unclipped stripes/bevel pixels from x=-20 through x=70 and y=20, with the
original straight alpha and order. Their transparent interiors prevent double
drawing. These source-derived assets inherit WTFPL-2.0.

`tests/reference/arkanoo_metal.py` checks both pinned source hashes and reproduces
the layers with real SDL. Six backdrop comparisons keep opaque body pixels exact
and explicitly record SDL software line-versus-texture-copy rounding differences
of one or two 8-bit levels outside the body. That numerical comparison applies to
the reference SDL renderer, not a bound on native/browser framebuffer output.
Exact full-frame alpha/color-space equivalence remains open. No original brick
geometry or outside coverage is deliberately omitted to hide that difference.
The signed runtime checks exercise both cold and warm full 200-metal boards
within the existing command, transfer and tick limits; the host asset cache
retains multiple immutable images within its existing 1 MiB pixel budget.


Recipe 0.1.25 restores the ice branch's f32 pulse, two inclusive edge glints,
three position-seeded refraction lines, five health-dependent crack segments,
and ten diagonals of the moving clipped shine. The scalar dependency-free Rand
compatibility stream now also implements the original i32 range sampler.
Gameplay and the editor share this path. The editor advances its own clock while
active, independently of the gameplay clock, as the original LevelEditor does.

`tests/reference/arkanoo_ice.py` compiles the unchanged original ice branch with
pinned Rand 0.8.5 and checks the original rendering/entities hashes. Its SDL
helper records 96 cases across twelve frame values (including f32 precision and
u64 maximum cases), four positions and both health states. Both alpha masks and
pulse colors match exactly in the signed native/Wasm tests. Glints retain
x=60/y=20 outside endpoints; only the diagonal band is clipped to 60×20.
Three ordered draw commands per brick keep a complete damaged 200-ice board
within unchanged command, pixel-byte and tick budgets. No new guest crates,
permissions, player tools or external assets are introduced.

The white overlays are grouped to stay within those budgets. Real SDL software
comparisons against the original per-line rendering over six backdrops record
up to three 8-bit levels of rounding difference. That reference bound does not
establish exact native/browser framebuffer equivalence; full-frame comparison
remains open. The source-derived rendering code retains WTFPL-2.0 attribution,
and the scalar Rand compatibility code retains its existing MIT notice.


Recipe 0.1.26 restores the explosive brick's complete original drawing sequence:
the f64 magma color pulse, moving orange core, top-half glossy fill, inclusive
edge glints, two diagonal reflections and eight-line moving clipped shine. The
original Rust SDL 0.37.0 wrapper clamps the apparent zero-height core rectangle
to one pixel; that moving highlight is preserved as a half-unit rectangle in
the projected canvas. The guest still has no Cargo dependencies. The shared
white-mask helper is unchanged in behavior for ice.

`tests/reference/arkanoo_fire.py` compiles the unchanged original branch using
real Rust SDL constructors from archives checked against the original lockfile
checksums, then rasterizes the recorded calls through real SDL. These crates and
Python/C tools are maintainer references, not player requirements or game crates.
The 64 cases cover sixteen frames, four positions, f32/f64 precision boundaries,
shine wrap and u64 maximum. Each case checks the real one-pixel core geometry,
pulse color and every byte of both alpha masks. Removing the core changes the
original output in all 384 backdrop comparisons. Grouped overlay rounding differs
from the original software SDL drawing by at most three 8-bit levels; this is not
a bound on native/browser framebuffer differences or complete frame parity.

Four commands per explosive brick retain all original drawing stages while
keeping full 200-brick and mixed ice/fire/metal boards within unchanged command,
image-byte and tick limits. Both gameplay and the editor share this path, with
the editor retaining its own active animation clock. Original art/fonts, bonus
and background presentation, audio/settings and full-frame/platform verification
remain tracked independently.


Recipe 0.1.27 restores the original glass bonus capsules, replacing the port's
flat blocks and letter labels. Ten source-derived PNGs preserve four original
tints, radial body transparency, top-left highlight, the interleaved double ring,
and all shadow/symbol passes. The duplicated white passes are retained in the
symbol result, including the long-paddle bar's combined six-pixel height. The
four legacy guest kinds remain long paddle, extra ball, ghost ball and rocket.
The capsule's inclusive point endpoints occupy 41×41 source pixels, while its
original collision box remains 40×40; destination geometry is integer-truncated
before half-scale projection.

`tests/reference/arkanoo_bonus.py` compiles the unchanged pinned drawing function
and original Bonus::rect with real SDL 0.37.0 constructors from verified archives.
Body, highlight and symbol layers retain straight original RGBA. Repeated ring
points are composed in a transparent OpenGL target and converted to straight RGBA
for the shared texture. All generated assets retain copyright (C) 2025 c0m4r and
WTFPL version 2; see assets/LICENSE-ARKANOO.txt and NOTICE. No external artwork,
fonts, player tools, game crates or permissions are added.

Six-background comparisons of the complete original and layered 41×41 capsule
agree within one 8-bit channel level in the original default OpenGL renderer.
Software SDL's texture-copy path differs by up to five levels, recorded separately.
These are explicit renderer measurements, not universal framebuffer equivalence.
Live native/browser comparisons retain original tint, transparent corners, bright
symbols, rim and highlight, including position changes. Exact nearest-sampling
boundaries allow adjacent original texels; full-frame rasterization remains open.
Four draws per item fit a cold/warm 200-item board in unchanged command, pixel-byte
and tick budgets, with shared immutable decoded images retained by the host cache.
Original falling motion, pickup rectangles and bonus behavior remain unchanged.

Rocket projectiles now use the original orange body, red line-drawn nose,
dark-orange fins and two flame colors. `assets/rocket.png` is generated from the
complete unchanged rocket draw loop in the pinned rendering.rs through real SDL
software rasterization (all source primitives are opaque). Its 10x20 pixels draw
at 5x10 with nearest sampling and original integer-truncated coordinates. One
shared cached image replaces the previous flat yellow rectangle without changing
collision or flight behavior. Copyright (C) 2025 c0m4r, WTFPL version 2.

The portal now follows the original draw_portal procedure: ten rings of 32
filled discs, accelerated shrinking, the expanding radial flash and white core,
and the final purple fade. Original f32 phase calculations, integer point
coordinates, draw order and 0/30/31/150/151/180/181/269/270 boundaries are retained.
The points are grouped through the bounded host image API; comparison over the
original dark backdrop measures up to four 8-bit levels of software-SDL rounding
difference. Nearest scaling and complete original-window framebuffer parity are
not established by the ordered-point fixtures. No new assets or dependencies.
Adapted from c0m4r's pinned rendering.rs, copyright 2025 c0m4r, WTFPL version 2.

Campaign levels 7–9 now use the original animated starfield, pulsing circles and
digital rain. Background point order, RGBA values, f32 timing and integer clipping
match the pinned original campaign branches. Their full canonical OpenGL frames
now also match exactly, superseding the earlier grouped-layer comparison. Star x addition explicitly wraps
like the original release build at very large frame counts. Visible background
pixels use non-overlapping 64-pixel tiles, preserving room for brick and particle
layers under the existing limits. Levels 1–6 image artwork, editor backgrounds
and complete-window background parity are unfinished. Adapted from
c0m4r's rendering.rs, copyright 2025 c0m4r, WTFPL version 2.

Endless levels selecting the original Snow, Smoke or Tornado themes now use their
original seeded positions, sizes, colors, motion and alpha. The original theme
selection seed and i32 sampler are preserved. Rand 0.8.5's separate u8 range
sampler is now reproduced for color draws; it consumes a different rejection
sequence from the i32 sampler. This scalar code remains covered by the existing
Rand MIT attribution. Dense smoke uses non-overlapping 128-pixel tiles; sparse
themes retain 64-pixel tiles. The unchanged source/real-Rand/SDL oracle supplies
33 cases across three levels per theme and frame extremes. Their earlier grouped-layer comparisons are now superseded by the exact
canonical default-OpenGL checks below. This does not complete whole-frame, edge, other-theme or platform parity.
Adapted from c0m4r's rendering.rs, copyright 2025 c0m4r, WTFPL version 2.

The original Fire and Deep Sea procedural backgrounds are also restored. Fire
retains its seeded flame positions, rising phase, four gradient bands, wiggle
and sizes. Deep Sea retains the original integer casts before scaling its light
shaft motion/width, ordered translucent bands and seeded drifting plankton.
Twenty-two unchanged-source/real-Rand/SDL cases cover three actual level seeds
per theme and extreme frame phases. Tiles share the existing density-based
64/128-pixel path; command, byte, time and memory limits are unchanged. Complete
scaled-window and platform parity still need verification. Adapted from c0m4r's rendering.rs,
copyright 2025 c0m4r, WTFPL version 2.

Supernova now uses the original pulsing core, ordered shockwave points and seeded
colored debris. Space Wars uses the original random firing intervals, offsets,
red/green probability and four-pixel-high laser bars. Frame-derived u64 addition
explicitly wraps like the original release build at extreme phases. The scalar
half-probability sampler matches Rand 0.8.5 and retains its existing MIT credit.
Twenty-two original-function/real-Rand/SDL cases cover these themes, including an
empty interval; four seeds check 1,024 probability draws. Whole-window parity remains open.
The existing grouping/sampling, full-window and platform limits still apply.
Adapted from c0m4r's rendering.rs, copyright 2025 c0m4r, WTFPL version 2.

Star Voyage now preserves the shared typed level RNG stream: theme selection,
u8 background colors, u64 star count, f32 speed multiplier and u8 tint are consumed
before per-star seeded draws. Eleven original-function/real-Rand/SDL cases verify
three level seeds and extreme phases. Complete viewport/platform parity remains open.

background.rs also contains an altered scalar Rust adaptation of SDL's default
point-line rasterizer and integer endpoint clipping. Copyright (C) 1997-2023
Sam Lantinga <slouken@libsdl.org>, zlib license; see assets/LICENSE-SDL.txt.
No SDL library is required by the game. A 32-case real SDL 2.30 OpenGL/software
baseline covers clipped/reversed/tie/degenerate/absent lines. The helper follows
the original default OpenGL coverage; software SDL omits the two zero-length
point cases. It is groundwork for remaining line-based themes, not completion
of those themes or proof of all window/backend combinations.

Kaleidoscope now integrates the attributed SDL point-line helper: five layers of
twelve rotating diamonds preserve the original 240 line commands, alternating
rotation, colors, endpoint casts and repeated corner blends. Eleven cases compare
the complete unchanged original procedure through actual SDL 2.30 OpenGL and
software line drawing. Command order/hash and all five per-layer pixel contribution maps match exactly.
The final known-background precompositor also matches complete logical default
OpenGL framebuffers byte-for-byte, including extreme repeated overlaps; software
SDL remains a separately measured renderer. Sparse tiles and limits are unchanged.
The helper also supports the later restored line-based procedural themes. Copyright 2025
c0m4r, WTFPL version 2; helper license remains assets/LICENSE-SDL.txt.

Rain and Warp now use the original seeded line commands, including rain slant,
speed/length, exponential warp distance, speed-based tint, alpha and tail length.
The upstream Rain & Thunder branch has its lightning code disabled; this port
preserves that active behavior. Twenty-two original-function/real-Rand cases
compare complete default OpenGL framebuffers exactly and verify 116 independent
ordered-segment-group pixel count maps. The original software renderer differs
by up to 15 RGB levels on overlapping faint warp streaks; it is recorded separately.
Rain uses 128-pixel tiles to retain all later brick/particle drawing under the
unchanged limits; other sparse themes keep their existing tiling. Whole-window parity
remains open. Copyright 2025 c0m4r, WTFPL version 2; helper license is LICENSE-SDL.txt.

Cyber Grid and Aurora Waves now retain the original typed shared RNG draws,
background and line colors, perspective/horizon motion and both aurora strip
orientations. Twenty-two actual seed/frame cases compare every ordered line and
complete canonical SDL 2.30 default OpenGL framebuffer exactly; 185 independent
48-command-group pixel contribution maps also match. Original software SDL
itself differs by up to six RGB levels and is preserved as a separate reference.
The existing attributed line helper and known-background compositor are reused;
all budgets and source restrictions remain unchanged. Complete viewport/platform parity
remains open. Copyright 2025 c0m4r, WTFPL version 2; helper license is LICENSE-SDL.txt.

Matrix Rain and Nebula Clouds now retain their original root/per-object typed RNG
streams, dimensions, colors, drift, phase and translucent rectangle order. Matrix
head arithmetic explicitly wraps as in the original release at extreme phases;
its resulting empty phases are preserved. Twenty-two original-function/real-Rand
cases compare all rectangle commands, 133 actual SDL 2.30 default-OpenGL grouped
contribution maps and complete canonical framebuffers exactly. Original software
SDL differs by up to 23 RGB levels after many faint cloud layers and remains a
separate measured reference. Rectangles composite directly into the logical
image, avoiding an expanded per-pixel list and retaining the original 16 MiB
memory limit and all drawing/time/source budgets. Complete viewport/platform parity remains open.
Adapted from c0m4r's rendering.rs, copyright 2025 c0m4r, WTFPL version 2.

Hex Pulse and Bubbles now retain the original grid/pulse RNG and motion,
rectangular bubble outlines, shine points, colors, casts and command order.
The default SDL outline path is reproduced as four endpoint-exclusive strips;
one-pixel degenerate behavior is preserved separately. This altered scalar Rust
outline adaptation of SDL RenderDrawLinesWithRectsF has the same Sam Lantinga
zlib attribution and assets/LICENSE-SDL.txt as the earlier line helper. Nineteen
real SDL outline baselines and 22 unchanged-original/real-Rand game cases verify
all commands, 306 independent grouped pixel contribution maps and every canonical
OpenGL framebuffer byte. The smallest original grid size is explicitly included.
Original software renderer differences are at most two RGB levels for these
cases. Complete viewport/platform parity remains open. Adapted game drawing: copyright 2025
c0m4r, WTFPL version 2; no SDL library is required by players.

Starry Night, Digital Circuit and Maze now restore the last three missing
procedural draw paths. Twinkling stars, satellite motion/blink, shooting-star
intervals/cutoffs, circuit nodes and independently selected moving connections,
maze diagonals and the moving solver retain original commands and order. Scalar
10%, 30% and 70% Bernoulli samplers preserve Rand 0.8.5's exact f64-derived u64
thresholds; 3,072 independent real-Rand samples verify their streams. Forty-three
unchanged-original/real-Rand/actual-SDL cases verify all raw commands, 349 grouped
contribution maps and complete canonical default OpenGL frames exactly. The
original software renderer differs by at most two RGB levels in these cases.
All twenty procedural paths are now adapted. The canonical checks below also
supersede the earlier point-background grouping bounds. Campaign 7–9 canonical
composition is also restored below; scaled-window/viewport parity and other
artwork remain open.
Copyright 2025 c0m4r, WTFPL version 2; existing Rand MIT and SDL zlib notices apply.

The eight earlier point/filled-rectangle themes now use exact known-background
composition: Star Voyage, Snow, Smoke, Tornado, Fire, Supernova, Space Wars and
Deep Sea. Eighty-eight unchanged-original/real-Rand/actual-SDL cases verify their
clipped ordered pixels, all 452 independently measured primitive-group maps and
complete canonical default OpenGL frames exactly. Original software SDL differs
by up to six RGB levels; this separate renderer difference is recorded, not a
port tolerance. The old full point-stream hashes and RNG vectors remain checked.
Smoke replays its 100 original rectangles directly instead of retaining a large
expanded point list. The original 16 MiB cap and all other budgets remain.
All twenty procedural themes now have exact canonical logical-frame evidence.
Complete viewport/window/platform behavior remains open. Copyright 2025 c0m4r, WTFPL version 2; existing notices apply.

Campaign levels 7–9 now also compose their original starfield, pulsing circles
and digital rain exactly against the original background. Twenty-one captures
of unchanged original point/fill commands through actual SDL 2.30 verify all
ordered pixels, 392 independent grouped contribution maps and every complete
canonical default OpenGL framebuffer byte. Original software renderer differences
are at most two RGB levels and remain recorded separately. Existing unclipped
point traces/hashes, original release overflow and all frame extremes remain
verified. All twenty procedural themes and all three animated campaign
backgrounds now have exact canonical logical-frame evidence. Original image and
editor backgrounds and complete window/viewport/platform fidelity remain open.
Copyright 2025 c0m4r, WTFPL version 2; existing notices apply.

Gameplay objects now use the host clip for the projected original world
(0,60,640,360), with status and control text outside that scope. Signed dense
bonus scenes verify all 800 layers between clip/reset commands under unchanged
budgets. All 80 logic checks, both rounds of 53 signed checks, native gameplay
and editor/system dialogs, and both browser profiles pass. Whole-window scaling,
original HUD placement and other original presentation requirements remain open.

The speed_font module contains rasterized Roboto Medium glyph coverage
from the original bundled font at size 26, captured with SDL_ttf 2.22.0.
Copyright 2011 The Roboto Project Authors; SIL Open Font License 1.1 is retained
in assets/LICENSE-ROBOTO.txt. The source compositor matches eighteen complete
original speed-label rasters byte-for-byte. Gameplay now uses the original speed/position calculations through bounded
filtered generated-image drawing. Signed pixel/geometry tests cover six speeds
and four fractional/negative poses. All six desktop and both browser profiles compare complete 521×17-pixel strips
against actual original-font SDL OpenGL output through four frame transitions:
283,424 RGB pixels, differences at most one on desktop and two in browsers.
Browser linear sprites use explicit bilinear sampling to avoid a confirmed
Canvas2D minification difference. Other font/HUD/window presentation remains open.

Campaign image backgrounds 1–6 are derived from the six original bundled PNGs.
The pinned upstream README credits c0m4r and says the backgrounds were generated
with Gemini 2.5 Flash Image; the original WTFPL notice in LICENSE-ARKANOO.txt
applies. A guarded SDL_image/OpenGL reference retains the original global linear
texture hint, 1280×720 logical sampling and alpha-64 gameplay brightness. Each
full-bright capture is stored in six bounded PNG tiles, reusable for different
original overlays. No original source pixels are reduced to fit the draw quota.
Bonus texture margins contain only zero RGBA pixels after cropping; the exact
original layers and offsets are preserved while reducing dense-scene transfers.
Both 55-check signed runs pass, including the full background-plus-800-layer
bonus transfer sum. Six desktop and two browser configurations verify six image
changes plus revisit: 4,928 samples include tile seams and world edges, with at
most one RGB level difference. Original bonus primitives over the restored image
also pass 480 comparisons within the existing three-level bound; rockets pass
672 comparisons within one level. Editor/menu presentation and whole-window
scaling remain open.

The editor now reuses the original six image backdrops with the original full-
brightness copy followed by a black alpha-153 overlay. Guarded actual SDL
captures verify every logical RGBA byte. Both 56-check signed runs pass with
full 200-metal-brick boards under unchanged budgets. Real Previous/Next buttons
and both wraparound directions pass in six desktop and two browser profiles:
9,152 sampled RGB pixels differ from original canonical values by at most one.
Background selection survives testing a board and returning to the editor.

Tiles retain exactly the same decoded pixels but now store opaque RGB scanlines
without PNG prediction or Deflate compression, avoiding repeated expensive
work outside the bounded 1 MiB LRU. A release-mode 512x512 decode benchmark
measured about 2.23 ms before and 0.14 ms after; no execution, image, drawing or
memory limit was increased. The larger source files are reflected in a 40 MiB
storage recommendation. Grid composition, original editor fonts/menu artwork
and whole-window scaling remain separately unfinished.

The editor grid now retains original inclusive endpoints and non-blended RGB.
Thirty-two opaque one-texel images reproduce 1201×1 horizontal and 1×201
vertical source-pixel footprints. Actual original SDL OpenGL grid/backdrop
captures verify all complete logical RGBA bytes for each of the six images.
Six desktop and two browser configurations pass real selection cycles, plus
three client sizes per desktop configuration: 7,320 RGB samples, difference at
most one with explicit nearest-boundary handling. A browser-only 43-level
fractional coverage mismatch is fixed with single-texel pixel-center coverage,
including the nearest sampler's lower-neighbor tie rule. No quotas or graphics
requirements increase. Browser CSS/full-viewport behavior and original editor
palette/lettering/menu presentation remain separately unfinished.


Recipe 0.1.47 restores the nine original editor palette colors, three-outline
white selection, ordinary/hover gray borders and original black Roboto Medium
26 digit glyphs. Existing OFL font attribution applies to these generated
rasters. A hash-guarded actual SDL oracle covers 28 complete palette states;
signed native/core-Wasm composition matches within one color level. Original
integer hover hitboxes and selected-border precedence are retained. Six Linux
desktop configurations compare every pixel of the original-size palette region
across three states and revisit: 329,664 RGB comparisons, maximum difference
one. Direct half-size SDL rendering, browser/full-window scaling, Colors label
and other editor/menu lettering remain unfinished. Budgets remain unchanged.


Recipe 0.1.48 restores 18 original editor static text rasters from the bundled
Roboto Medium 26 font: title, Colors label, toolbar captions, background
counter and editor/browser instructions. Existing SIL OFL attribution applies.
All complete RGBA bytes and regeneration match independent guarded SDL_ttf
captures. Toolbar background alpha, RGB, opaque borders, centered captions and
integer hover boxes follow the pinned original. A long label uses bounded
filtered pieces with source overlap and destination clipping; limits stay
unchanged. Six original-size desktop configurations pass 1,760,088 complete
label/toolbar RGB comparisons within two levels. Separate eight-player
filtered-font seam/move/revisit checks pass 408,960 comparisons within one.
Dynamic names/messages, browser panel/rows and complete editor/window scaling
remain unfinished.


Recipe 0.1.49 restores original dynamic editor name, saved-pattern row and
message lettering with all 1,321 captured Roboto Medium 26 glyphs. Names retain
case and Unicode contents, original suffixes/colors/positions and viewport
clipping rather than the former uppercase/question-mark/78-character display.
The font's normalization, contextual substitutions and GPOS kerning are stored
as explicit-endian data; font-aware normalization is adapted to this pinned
font from HarfBuzz (Copyright 2011,2012 Google, Inc.; see
assets/LICENSE-HARFBUZZ.txt). Unicode data uses the included Unicode License V3
(assets/LICENSE-UNICODE.txt); existing SIL OFL font attribution also applies.
Original missing-glyph behavior is retained, including unsupported characters.

The independent SDL_ttf oracle covers 3,268 cases: 3,259 complete RGBA surfaces
and nine empty results. Cases include every non-NUL mapped scalar, combined
accents, kerning, negative bearings, long names and a 1,205-case compound
joiner/variation audit. All 4,174 default-ignorable candidates are checked
against actual original HarfBuzz, including the nine exceptions. Full source
pixel comparisons and generation match. Signed native/core-Wasm probes verify
source bytes, placement, color and clipping for names, rows and messages;
both 60-check suites, 86 logic tests and native/browser file flows pass.
Eight-player whole-region Unicode comparisons total 1,576,960 RGB pixels,
with maximum desktop difference two/browser one. A browser clip-antialiasing
error that halved some glyph edges is corrected with independently rounded
hard scissor bounds; shared clip/image/shader/resize checks remain exact.
End users need no Python, C, FreeType, HarfBuzz or font dependencies for this
rendering. Budgets remain unchanged. Browser panel/row geometry, full editor
composition and whole-window/CSS scaling remain separate unfinished work.


Recipe 0.1.50 restores original saved-pattern browser composition: black
alpha-200 world dimming, opaque panel, border, normal/selected row colors and
original font placement/order. Empty lists use the original empty panel;
existing extra paging is retained. An independent guarded SDL oracle records
twelve complete canonical frames (empty, one entry and all ten selections).
Signed native/core-Wasm full-frame composition matches within two levels,
including a dense 200-metal board plus ten mixed/long-name rows without dropped
browser commands or raised budgets. Six original-size desktop configurations
pass 22,118,400 complete-world RGB comparisons within one level. Both browser
profiles pass 1,843,200 comparisons with a separately captured smaller native
canvas within two levels; this is backend consistency, not direct scaled-
original equivalence. Native single-texel image bounds now share the browser's
physical-pixel rule, fixing disappearing half-pixel borders. Shared clip/image/
shader/resize and editor-grid regressions pass afterward. Both 61-check suites
and native/core/component browser editor/file workflows pass. Full editor/menu
frames, arbitrary viewport/CSS scaling and untested platforms remain unfinished.

Recipe 0.1.51 restores the implemented main/SFX/fullscreen/back menu layer's
original Roboto captions, layout, hover fills, borders, slider percentages and
background-one dimming. `menu-background1-{0..5}.png` are overlapping tiles of
the original WTFPL background, captured at the canonical SDL viewport; users
need no capture/compiler tools. The independent 38-frame partial-layer oracle
and tiles reproduce exactly. Signed native/core-Wasm checks and six desktop/two
browser rendering configurations pass within unchanged budgets and a two-level
pixel tolerance. The port retains its accessible Back-button extension. Gravity
mode persistence is checked without an invented visual stripe; accepted
fullscreen requests update the caption and survive run restart. External window
state readback, original music/resolution/VSync controls, project/version display,
splash/HUD artwork, arbitrary original scaling and other platforms remain open.
The advisory storage estimate is 48 MiB after measuring 43.0 MiB with package,
recovery copy and native/Wasm builds; hard memory and drawing limits are unchanged.
Final native and both browser-profile gameplay/editor and player-selected file
import/cancel/export workflows pass. Publication is source only.

Recipe 0.1.52 restores the original startup artwork and white-background fade.
`splash-{0..5}.png` are overlapping canonical SDL captures of upstream
`assets/antigravity.webp` at revision
`9d613e78c9ed5657ae7fda9206cd4a82a893b7c8` (source SHA-256
`663904220eeb0bec0cc4b729b617808f0d40fc7de2501672435dccfda84d347e`).
This upstream development-tool acknowledgment image is preserved as game
content under the upstream project's WTFPL terms. It describes the upstream
game's credits. The six tiles and independent 17-frame oracle reproduce
exactly; all 271 original alpha values are checked. Six desktop configurations
match canonical SDL RGB frames exactly, and both browser profiles match a
separate smaller native capture within two levels. End users need no capture,
image-conversion or compiler tools for this artwork. Timer/skip/restart behavior
remains verified separately. HUD/end-state artwork, recorded audio/music,
remaining settings and arbitrary original scaling remain unfinished.
Both 63-check signed suites and native/core/component browser gameplay/editor
and file workflows pass. The advisory storage estimate is 56 MiB after measuring
48.3 MiB with the package, recovery copy and native/Wasm builds. Execution and
memory/drawing budgets remain unchanged. Publication is source only.

Recipe 0.1.53 restores original level-complete, game-over and campaign-victory
overlays: full-world dimming, Roboto captions, colors, centering, final scores
and original instructions. The game-over green VICTORY! branch remains distinct
from campaign completion. Zero and ten-digit u32 scores/levels use explicit
portable decimal formatting. A guarded independent SDL oracle supplies 28 full
contrast-backdrop overlay frames and reproduces byte-identically. Six desktop
configurations match canonical overlay frames within one level, and both web
profiles match a separate smaller native capture within two levels. Full-scene
suffix checks cover every end state, including 200-metal-board and animated
background scenes, without increasing command/image/memory/time limits. This
restores the end-screen layer; original HUD/FPS, audio/music, remaining settings,
full-window scaling and other platforms are separate unfinished requirements.
Particle drawing now reuses each shard's sine/cosine and avoids allocating/copying
fully transparent temporary tiles. Visible pixel data and draw commands remain
unchanged. All 87 logic checks and both 64-check signed suites pass with the
original runtime limits and default parallel execution; strict lint passes.
Final native and core/component browser gameplay/editor and player-selected
file import/cancel/export workflows pass. Publication is source only.


Recipe 0.1.54 restores original score, lives/stolen heart, level/TEST,
maximum-speed record pulse, gravity and FPS placement/Roboto lettering. The FPS
counter uses the host's per-tick relative monotonic sample and the original
sliding one-second history. A guarded SDL/TTF reference supplies 29 full HUD
frames; canonical desktop and smaller browser comparisons preserve existing
pixel tolerances. Original heart pixels use straight-alpha linear sampling.

To retain the complete original HUD in dense scenes, fire-brick layers compose
at original 61x21 resolution once per frame and bonus icons are captured from
the unchanged original SDL primitives into 41x41 source textures. The bonus-icon
PNGs derive from the same upstream rendering source and license credited above.
Original per-layer reference captures remain available; full images are checked
over six backgrounds without relaxing the earlier three-level tolerance.
Sparse 128-pixel particle tiles preserve all visible shards. Runtime memory,
execution, command and image limits remain unchanged. HUD-layer fidelity does
not establish arbitrary original viewport, recorded audio/music, remaining
settings or platform completion. Publication is source only.

Final verification passes all 90 logic checks and both 66-check signed suites at
default parallelism, plus native and core/component browser gameplay/editor and
player-selected file import/cancel/export workflows. All 924 shared clip/image/
shader/resize samples remain exact and eight end-screen comparisons still pass.
Strict lint and formatting pass; no runtime threshold is relaxed.
HUD, filtered-edge, fire and bonus fixtures reproduce byte-identically.


Recipe 0.1.55 restores the original ammo-gated cannon barrel/highlight and
blinking `press space to launch` caption, including original integer centering,
colors, source-world sizes, Roboto26 and even-30-frame blink intervals. It draws
after the paddle and restores the world clip before later objects. An independent
SHA-guarded oracle executes the unchanged original cannon branch with original
SDL 0.37 and actual SDL_ttf at both 1280x720 and 640x360, covering 60 states.
Dense full-renderer checks retain the complete cue and all three original heart
textures with 200 fire bricks and 3,000 corner particles under unchanged limits.
Actual scaled checks exposed rectangle edge differences; the host now follows
measured SDL pixel-center coverage in both desktop and browser game drawing.
Recorded audio/music, remaining settings, arbitrary whole-window scaling and
untested platforms remain separate work. Publication is source only.

Both 67-check signed suites pass at default parallelism and unchanged limits,
with native and core/component browser gameplay/editor/file workflows passing.
All six desktop configurations match original cannon frames at both sizes
within the existing two-level tolerance; both browser profiles match original
half-size frames within one. Four selected states plus revisit run through each
graphical path; all 60 states have signed canonical runtime checks. All 120
original captures and the nine solid-edge fixtures reproduce byte-identically.
Shared clipping/blending and original HUD/menu/browser/end-state graphics
regressions pass without changing thresholds; strict lint and formatting pass.


Recipe 0.1.56 makes the original Fullscreen/Windowed caption follow the host's
observed own-window state, including saved fullscreen startup and external exits.
An accepted toggle no longer changes the caption speculatively. Unavailable
observation preserves its last confirmed value. The bounded, permission-gated
SDK works through native/core/component and browser bindings without guest
handles, OS access or dependencies. Actual X11/window restoration/F11 and browser
API/native-Escape/restart paths are verified; other native platforms remain open.
Trusted X11 UI observation and idempotent own-window requests work around winit's
stale cache after external WM changes, using the existing display connection and
bounded property reads. No runtime limit or input gesture requirement is relaxed.
Recorded audio/music, remaining settings/project/version controls, arbitrary
whole-game scaling and untested platforms remain unfinished. Source only.

Dense-scene deadline checks also prompted lossless buffer changes: particles
compose directly into occupied 128-pixel tiles in the original grid/order, with
unchanged RGBA bytes and charges. The four-corner 3,000-shard case avoids a
3,686,400-byte temporary while retaining the same 212,992 bytes of visible tiles.
Background initialization repeats the original RGBA byte pattern with bounded
copies. All 91 logic checks and both 67-check signed suites pass at default
parallelism and unchanged limits, plus native/core/component browser gameplay,
editor and file workflows. Saved-window regressions and actual fullscreen/exit/
reopen tests pass. Host Windows32/64 compile checks pass, not platform gameplay.
Pending native fullscreen toggles wait for a known host state; unavailable is
never treated as windowed. Final controller/window/browser regressions pass.


Recipe 0.1.57 restores the original requested sound categories: Bounce for
normal/final ice destruction, BreakingGlass for ice cracks and rockets,
Explosion for explosive-brick blasts and Oh for a lost final ball. Removed
non-original launch, bonus, level-complete and portal-activation requests.
The unchanged pinned original physics fragments now record last-callback
selection for ten cases/thirteen frames, preserving all earlier physics fields.
Original main-loop last-callback behavior within a physics update is retained.
Recordings, overlapping playback, complete input/update audio ordering and music
remain unfinished; current effects are still synthesized, not original assets.

Text rasterization now skips transparent RLE runs and computes row coordinates
per run rather than per pixel. RGB initialization repeats the exact four-byte
pattern. A maintainer old/new comparison passes 122,870 cases including clipping,
empty slices and hidden RGB. The cannon-caption raster benchmark improved about
22 percent locally; this is not a platform performance guarantee. Opaque particle
contributions now use the exact simplified original blend equation. Exhaustive
alpha/color checks preserve every tested result and original SDL image fixtures
remain exact. All 93 logic checks and both 68-check signed suites pass at default
parallelism and unchanged deadlines/budgets/tolerances; strict lint and formatting
pass. This round did not repeat graphical window/platform workflows. Source only.


Recipe 0.1.58 adds a four-slot 48 kHz stereo PCM effects mixer. Clips retain
independent positions across output frames, reuse the first free channel and
reject a fifth active effect. Gain, per-channel saturation and channel order
match actual SDL_MixAudioFormat output. Muting prevents new effects while
existing effects advance silently; a run reset preserves active clips, as the
original AudioManager outlives Game resets. Event sounds are captured after
each input; controller fire is captured before physics. The final physics
callback remains separate, so a rocket launch and collision can both sound.

Production still supplies the temporary synthesized one-frame tones. Long
stereo fixture clips prove the mixer path, not restoration of the original
recordings or their durations. Original recordings/provenance, compressed asset
playback, music and full callback/wall-clock timing remain open. No assets or
binary packages are added. All 96 logic checks, four source-attack checks and
both 69-check signed suites pass without changing caps, parallelism or pixel
tolerances. Expanded six-case signed tests pass in explicitly selected native
and Wasm targets; actual core/component Chromium WebAudio buffers preserve
stereo tails, free-channel reuse, muted progress and keyboard/controller
rocket-plus-brick overlap. Strict lint and formatting pass. Native graphical
window/device and broader platform workflows were not repeated this round.


## Restored recorded effects

The original life-loss request uses "Drop Sound Effect" by 11325622:
https://pixabay.com/sound-effects/film-special-effects-drop-sound-effect-240899/
The original explosion request uses "Swish Swoosh Woosh SFX-27" by
StereogenicStudio:
https://pixabay.com/sound-effects/film-special-effects-swish-swoosh-woosh-sfx-27-357164/
Both source pages identify the Pixabay Content License; these recordings do not
use the port's code license. See assets/LICENSE-PIXABAY-AUDIO.txt and
https://pixabay.com/service/terms/#license . They accompany the game adaptation,
not a standalone sound library.

The pinned original MP3 files were decoded through SDL_mixer at the original
44,100 Hz signed 16-bit stereo mixer format, then converted by SDL_AudioCVT to
the host's 48,000 Hz format. No trimming, normalization or looping was applied.
The resulting WAVs are loaded through bounded package-asset reads, with explicit
little-endian samples. Players need no SDL_mixer, Python, FFmpeg or MP3 decoder.
The maintainer reference and complete source/PCM/WAV hashes are in
https://github.com/centrexity/GamePKG/blob/main/tests/fixtures/arkanoo-recorded-effects.json
and the reproducible conversion tool is
https://github.com/centrexity/GamePKG/blob/main/tests/reference/arkanoo_effects.py .

The clips contain 159,242 and 254,969 stereo frames respectively, retaining their
independent channel data. Their decoded PCM totals 1,656,844 bytes. Active voices
share immutable cached PCM instead of copying a recording per voice. The loader
accepts the canonical 48 kHz stereo PCM16 WAV layout, bounds a clip to 2 MiB and
rejects truncated headers/data, trailing bytes and inconsistent lengths. Failed
loads stay silent rather than reverting to a tone. The other three effects are
still explicitly synthesized; original music and full asynchronous timing remain
open.


Recipe 0.1.59 validation: all 99 logic checks and both 71-check signed native/
Wasm suites pass with the original memory, time, sample and image budgets and
default test parallelism. Full-clip checks compare every original prepared PCM
sample, including distinct stereo channels, exact frame counts and final zero
padding. Four simultaneous voices sharing both recordings stay within the
unchanged memory limit. Invalid headers, oversized lengths, short data and
trailing bytes are rejected atomically; cached failures do not repeatedly read.
The original conversion reproduces the committed hashes byte-for-byte.

Actual core/component Chromium WebAudio buffers also match both entire clips.
That longer test exposed browser timing drift: a 16 ms delay can run faster than
60 Hz and fill the audio queue. Browser fixed-step scheduling now retains its
fractional 1/60-second phase and avoids catch-up bursts after lateness. The
100 ms audio queue remains unchanged. A 600-tick quantized-timer regression and
existing browser input/window/clock/stereo checks pass, along with both portable
browser gameplay/integrity/shutdown regressions. Strict lint and formatting pass.
This does not establish new physical-device, graphical-window or native platform
coverage, or finish music and fully asynchronous playback timing. Source only.


Recipe 0.1.60 restores the remaining exact upstream ball.mp3, breaking-glass.mp3
and load.mp3 recordings as bounce.wav, glass.wav and launch.wav. The pinned
upstream README identifies sounds/music as Pixabay content. History records the
generic filenames, with no individual artist/item links for these three files;
that credit basis is explicit in the metadata and audio license notice. Creator
fields are null when upstream did not supply a name. These files are not assigned
to an invented artist or relabeled under the code license. Complete original
MP3, prepared WAV and PCM hashes remain reproducible through the SDL reference.

All five effects now use their recordings; the synthesized generator and old
render::sound path are removed. Initial silence, original durations and stereo
content remain intact, including the mono glass source duplicated by SDL into
both output channels. Five immutable cached clips total 2,208,852 decoded bytes.
A shared 4 MiB resident budget retains the previous worst-case two-clip bound,
and each clip still has the same 2 MiB bound; four voices share the cached data.
No frame/sample/image/memory/deadline limit is increased.

Gain and overlap checks compare an audible window after the originals' initial
silence, and the input-plus-physics test compares complete combined clip output.
Invalid effect IDs are rejected before reads/indexing; cache exhaustion rejects
before allocating another clip. All five full PCM streams and cached playback
remain architecture-independent through explicit little-endian samples. Music,
fully asynchronous timing, individual artist/item identification for the three
generic source files and untested native platforms remain open.


Final 0.1.60 checks: 100 logic tests and both 71-check signed native/Wasm suites
pass at unchanged limits, tolerances and default parallelism. Every prepared PCM
sample is checked through all five clips in one session, retaining all five
cached recordings. Actual core/component WebAudio buffers match all five full
PCM hashes and endings. Audible-window and complete mixed-output checks verify
gain, original silence, keyboard/controller requests and independent channel
content. The SDL conversion metadata reproduces byte-for-byte; strict lint,
formatting and the browser fractional-clock regression pass. No new physical
device, native graphical-window or broader platform coverage is claimed.


## Original project-homepage button

Recipe 0.1.61 restores the original main menu's `Github` button at original
rectangle (1170, 670, 100, 40), projected to (585, 395, 50, 20). Its original
normal/hover rendering and complete unchanged-source hit-test results are checked
with the other implemented controls. The original click maps to the signed
manifest homepage via host `window_control(4, 0)`; game code supplies no URL and
spawns no process. Fresh input permission and successful tick/persistence are
required, and duplicate requests within one tick are rejected.

Private tests click the actual production handler in native/core/component
desktop windows with a recording browser executable, then both browser exports
with a loopback-mapped test hostname. Browser targets have no opener, requests
are consumed once, and unfocused/invalid/failed-tick requests do not navigate.
Failed browser save/history storage also suppresses navigation without replay
after recovery; a fresh click remains available.
The host version stays 0.1.17; this new operation requires rebuilding the host
from current source. Source recipes only are updated, with no new signed package
artifacts or release. Music/resolution/VSync settings, the version label and
remaining menu/game presentation stay unfinished.


A source-only host-music playlist controller is prepared in source/src/music.rs.
Its observable gain/mute/pause/play/completion/stop/retry states and random choices
match 32 steps run against the unchanged pinned original audio.rs. It uses only
GamePKG controls and virtual package paths; it contains no decoder, game crate,
filesystem or network access. Bounded playlist input, staged completion handling
and missing/invalid input cases have host tests; actual isolated native/Wasm
source-policy builds pass. It is not yet activated by the port entry point: the
recorded-effect migration, persisted music settings and full port playback
qualification still need integration. No recordings or package release accompany
this source preparation.

Recipe 0.1.65 restores the original desktop resolution choices (1280x720,
1366x768, 1600x900, 1920x1080 and 2560x1440), their original hit rectangles,
selection highlight and Keep/Revert confirmation. Original Roboto labels and
the original 300-frame countdown are retained; the trusted host performs the
actual resize, confirmation, rollback and timeout. Committed host observations
reconcile the menu each tick, including failed requests and automatic expiry.
Actual client dimensions select the highlight; arbitrary sizes do not pretend
to be a preset. The menu blocks its other buttons during confirmation, as in
the original. Size selection is available only in desktop windowed mode; the
menu explains returning from fullscreen. Browser/headless hosts hide these
desktop controls. Original source is unchanged. VSync, canonical music delivery,
remaining presentation and the wider parity ledger remain open.

The already-selected resolution is a no-op, as in the original main loop, and
restarting a run preserves an active preview/countdown. Verification includes
225 unchanged-original menu boundary cases, 62 independent complete SDL menu
frames, 117 unit tests and 71 signed runtime regressions in each of native and
Wasm. Real Linux native/core/component windows pass original mouse selection,
Keep, Revert, confirmed-size reopen, expiry and reuse after expiry. The final
full host suite passes 664 tests with zero failures (151 gated tests ignored);
strict release lint and formatting pass. The private recorded-music preparation
still builds both targets, without publishing any recordings. The main-menu
Version: 0.5.0 caption was restored in recipe 0.1.64.

Recipe 0.1.66 restores the original VSync setting. Its original button rectangle,
Roboto caption, hover and ON/OFF preference are retained; the change applies
after restarting the game, as in the original main loop. The original restart
notice is placed at original y=190 to avoid its overlap with the resolution
header. No game source receives a graphics context or settings-file path. The
host owns the per-game/creator/library preference, defaults older records to ON,
requires signed input plus a fresh action to change it, and rolls back a failed
tick. Browser/headless hosts hide this desktop control. At launch the separate
game window selects OpenGL swap interval 1/0 or Vulkan strict FIFO/automatic
non-VSync; a library window keeps its independent default.

Independent source-guarded SDL/TTF references now cover 68 complete menu frames
and 243 original button boundary cases. All 71 signed runtime regressions pass
in both native and Wasm; real Linux X11 native/core/component windows pass
ON/OFF/ON across restart on both graphics backends. A private forwarding observer
records actual swap-interval calls and successful Vulkan swapchain creation;
it never substitutes a success or changes an API argument. These calls establish
the requested backend configuration; drivers and compositors control physical
presentation. The final host suite passes 669 tests, zero failures (151 gated
tests ignored), with strict lint and formatting clean. Existing resolution and
window-state GUI checks still pass. The private full recorded-music preparation
also builds both targets. Canonical music delivery, additional platform/GPU
qualification and the wider parity scope remain open; no recordings are published.

Recipe 0.1.67 extends the font-derived viewport path to long mixed-class runs
whose unconsumed marks all draw the original missing glyph. It preserves the
complete saved UTF-8 and original canonical ordering/composition. Each new class
must leave the base unchanged; subsequent equal-class marks retain the original
composition blocking. Thus A/cedilla/ogonek remains supported, while a late
composing cedilla on S retains full shaping. No name, media, scene or budget is
truncated or enlarged.

Four reordered/blocked contexts and 5,940 ordered representative class pairs
match the eager shaper and exact visible rasters. A complete one-MiB name using
alternating cedilla/U+0315 passes private native/core/component edit/save/restart
with all original recordings retained, and both actual browser profiles preserve
the exact name and maximum collection through persistent reload. Seventeen font
regressions also pass as a real 32-bit process. The standard suite passes 671
tests with zero failures, and all 71 canonical native and 71 Wasm runtime checks
still pass. This clears a specific recorded-audio migration blocker; general
giant-cluster shaping, canonical music activation and wider parity remain open.

Recipe 0.1.68 adds bounded display normalization for long missing-mark runs
whose late mark composes with an ASCII starter, such as S becoming Ş after a
late cedilla. Class counts and three leading candidates retain the original
canonical ordering and composition blocking. The complete pinned font graph
has at most two supported composition transitions; tests check that bound and
the absence of cycles. Original saved text is untouched. Only display text may
use the final composed scalar and a shaper-verified representative of glyph zero.
Actual glyph bounds, lookahead and complete vertical extents are checked before
using a shortened display; unproven cases keep full shaping. Single-piece pattern
rows now use this preparation too, alongside segmented names and editor headers.

All 262 eligible ASCII missing-mark composition cases match the eager shaper and
exact visible rasters. An exact one-MiB late-composition archive edits, saves and
reloads in private native/core/component and actual Chromium core/component full
ports with all original media retained. The prior mixed-class maximum archive
still passes. No limits or source-policy exceptions change. All 124 port unit
tests, 21 real 32-bit font regressions, 675 standard host tests and both 71-case
canonical native/Wasm runtime suites pass, with strict lint and formatting clean.
Canonical music activation, other giant shaping/extents cases and wider parity
remain open; no recordings are published.

## One source for effect-only and complete recorded-audio packages

Recipe 0.1.70 removes the compile-time audio-profile switch. The canonical entry
point reads the package playlist once; a nonempty validated playlist selects
shared host effects and music, while a missing playlist or one with no accepted songs retains the existing
effect-only path. Playlist parsing and host permissions remain
bounded. Complete recorded-audio assembly copies verified assets and declares
its existing memory shares without modifying any game source file.

The recorded profile preserves its effect routing through run restart, including
host refusal without guest PCM fallback. Saved music mute/volume and ARK4
preferences remain intact. All 128 logic tests and five controller tests pass;
both complete 72-test effect-only native/Wasm suites pass. The same unmodified
source also passes full private native/core/component music, effect and maximum
Unicode-name editing/save/restart checks, plus actual Chromium core/component
playback and persistent reload. Six dense scenes with 200 blocks, 3000 particles
and a full 1 MiB/5041-pattern archive match every reference drawing command and
image byte in all desktop and browser profiles under unchanged resource limits.
The browser retains all 76 lossless images, five effects and nine music tracks.

This supersedes the earlier source-preparation and activation notes. Complete
asset delivery remains unfinished; the default asset set remains effect-only.
Broader text-shaping, campaign, device and platform qualification remains open.

## Playlist failure preserves original recorded effects

Recipe 0.1.71 distinguishes a missing playlist from a packaged playlist with no
usable tracks. Empty, invalid UTF-8, invalid-path and interrupted playlists never
start partial music, but a playlist observed in the package retains the recorded
effects path. This corrects 0.1.70's selection by accepted-song count: its empty
playlist selected the legacy 48 kHz reader for the new 44.1 kHz recordings and
silenced effects. Missing playlists retain the existing effect-only behavior.

A new signed regression reproduces that silence in the prior source on native,
core and component runtimes. All three malformed/empty playlist cases now retain
audible original effects on initial load and fresh relaunch, with stopped music
and no guest PCM fallback. Actual Chromium core/component checks also retain
host effects through restart and release the complete memory budget at Stop.
CI runs every case on desktop and browser. Six controller tests and all 128
logic tests pass, including interrupted-read handling. Normal full-media
native/core/component and both browser profiles still pass playback, saved
settings and maximum-name/collection editing and persistent reload. Both 72-test
effect-only native/Wasm suites and all dense-scene/full-save comparisons pass.
No recordings, limits, test concurrency or saved name capacity are reduced.
Default complete asset delivery and broader campaign/platform parity remain open.

## Legacy save validation

Recipe 0.1.72 validates the gravity word before the ARK1 decoder's early return.
Only 0 and 1 are valid, matching every archive writer and the later formats.
Previously ARK1 accepted other values and cleared current patterns/preferences;
subsequent settings changes could migrate the corrupt record instead of
preserving it. A red regression reproduces acceptance of flag 2 before the fix.
Borrowed and owned decoding now reject 286 invalid flag cases without replacing
existing pattern bytes or audio settings. Valid ARK1/ARK2 migration remains intact.

All 129 logic and six music-controller tests pass. Three unchanged archive test
bodies also pass as an actual ELF32 program, including full-size archives. Both
72-test native/Wasm suites verify six damaged ARK1–ARK4 records remain unchanged
while displaying the existing warning. The full private recorded-audio package
preserves those records and retains audible music/mute/resume in native, core
and component runtimes. Actual Chromium core/component tests write the damaged
bytes into IndexedDB and verify them after Stop, with visible warnings and
functional music controls. Full-size name/collection persistence also passes.
CI includes the browser database matrix and ELF32 checks.

A longer repeated browser run did expose an intermittent music Failed state
during mute after several fresh sessions. A subsequent diagnostic run passes,
but that does not establish a fix. Test-only bounded decoder request/error/timing
traces are retained; the decoder cause remains open, alongside complete asset
delivery and wider campaign/platform parity. No execution/memory limits or
playback assertions were relaxed.

## Browser music handoff and frame scheduling

Current host exports fix two browser playback problems found during repeated
full original-media sessions. Audio transport now preserves generation-owned
read credit across a late begin and safely re-sends the same request if it was
ignored before decoder attachment. Duplicate requests never decode twice;
changed/stale credit still fails validation. Deterministic real MessageChannel
and production AudioWorklet tests cover both orderings, paused/playing state,
complete source frames and budget release.

The renderer also prepares offscreen frames cooperatively so long drawing tasks
do not starve main-thread decoder replies. Draw order, pixels, complete scenes
and persistence gates remain unchanged; cancelled/replaced frames are serialized
on the same bounded surface. Actual synchronous/cooperative browser output matches
the pinned renderer byte for byte. Both complete browser profiles pass three full
six-record restart/mute/save matrices with no recorded Failed snapshot, retaining
all original images, effects, music and maximum name/collection saves. Dense scene
and generic player lifetime/persistence regressions also pass under the existing
execution and memory limits. These are host changes; recipe source and assets
remain unchanged. Broader delivery and platform verification remain open.

## Host browser graphics recovery

Current host exports recover lost filtered-image and shader contexts on fresh
Play without reloading the page. The host waits for cancelled offscreen drawing
and old audio shutdown before replacing lost caches, reconstructs shader IDs
cleanly, and releases partial shader/context initialization on failure. Healthy
contexts remain reusable. Lost graphics cannot publish a partial prepared frame.

The signed core/component recovery fixture passes 18 real Chromium cases with
synthetic music, distinct clipped shader programs, a sampled texture and filtered
pixels. It checks
initial setup failures, unavailable/failed compilation during retry, suspended
render cancellation and simultaneous loss of both WebGL contexts. Exact whole
frame hashes, advancing music, complete audio-budget release and bounded healthy
context counts pass. This is host behavior; original port source/assets remain
unchanged. Broader native/device/browser graphics-reset qualification remains open.
