# Arkanoo for GamePKG

Adapted from c0m4r/arkanoo: https://github.com/c0m4r/arkanoo
Upstream revision: 9d613e78c9ed5657ae7fda9206cd4a82a893b7c8
Copyright (C) 2025 c0m4r. Upstream code: WTFPL version 2.
Upstream credits: c0m4r, Gemini 3 Pro, Claude Sonnet/Opus 4.5.

This port adapts the nine campaign layout predicates from entities.rs,
paddle/ball motion and spin, overlap collision response, scoring, and the
long-paddle, extra-ball, ghost-ball and rocket mechanics from entities.rs and
game.rs. The engine integration has been rewritten for GamePKG's safe APIs.

Rendering, bitmap lettering and synthesized effects are new; upstream Pixabay
music/sound, fonts, logos and generated background art are not redistributed.
The port uses a 640x480 canvas, bounded particles/balls/bonuses, swept substeps,
and a deterministic local generator instead of SDL2, rand or filesystem access.
Endless levels use new bounded procedural patterns. The upstream editor,
mouse controls, original menus and music are not included. Collision geometry,
launch/bonus timing, collision ordering and editor block types still need
fidelity work; the current 640x480 presentation is not yet the original world
geometry. Level progression and several entity counts also retain port limits.

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
fixtures. Particle lifetimes/shapes and movement/paddle/block collision ordering
still differ from upstream; this does not establish complete physics parity.
