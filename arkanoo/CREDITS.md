# Arkanoo for GamePKG

Adapted from c0m4r/arkanoo: https://github.com/c0m4r/arkanoo
Upstream revision: 9d613e78c9ed5657ae7fda9206cd4a82a893b7c8
Copyright (C) 2025 c0m4r. Upstream code: WTFPL version 2.
Upstream credits: c0m4r, Gemini 3 Pro, Claude Sonnet/Opus 4.5.

This port adapts nine campaign and twelve procedural layout rules from entities.rs,
paddle/ball motion and spin, overlap collision response, scoring, and the
long-paddle, extra-ball, ghost-ball and rocket mechanics from entities.rs and
game.rs. The engine integration has been rewritten for GamePKG's safe APIs.

Rendering, bitmap lettering and synthesized effects are new; upstream Pixabay
music/sound, fonts, logos and generated background art are not redistributed.
The port uses a 640x480 canvas and GamePKG host randomness instead of SDL2,
rand or filesystem access. Launch directions use rejection sampling, and bonus
selection uses independent probability/weight draws.
Endless levels now use the original twelve procedural patterns. The upstream editor,
mouse controls, original menus and music are not included. Particle/trail effects,
the penguin/heart-loss animation and custom-pattern import/selection still need
fidelity work.

Controls: Left/Right or A/D move; Up/W launches, fires a rocket, and confirms
start/next level/restart; Down/S pauses/resumes; V/controller X toggles gravity. Three lives, nine campaign
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
Gravity selection uses the formerly reserved word of the existing 16-byte ARK1
save, retaining legacy best/unlocked fields and old saves. Full run saving and
the broader physics/geometry differences above remain unfinished.

Ball-to-ball contacts now restore the original strict distance test, upward
minimum-speed ejection and horizontal separation, including coincident balls,
simultaneous pairs and portal exclusion. Seven fixtures execute the unchanged
contact block from the pinned original and match the port's half-scale velocities
and 36-particle counts. The cyan shockwave is rendered by the real native/Wasm
fixtures. Particle lifetimes/shapes still differ from upstream; this does not establish
complete physics parity.

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
campaign/endless levels; this port does the same. The validated type-setting
logic is ready for editor integration. The editor UI/import workflow itself
remains unfinished; these tests do not claim it is available yet.

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
all 106 rendered boards, including brick coordinates. Custom patterns in the original endless selection
pool still require the editor's scoped save/import workflow; that remains open.
