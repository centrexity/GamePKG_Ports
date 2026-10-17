# 2048 for GamePKG

GamePKG adaptation of Adrien Ball’s 2048-rs.
Upstream: https://github.com/adrienball/2048-rs
Revision: 9e972c43e51b153f66b31a08c37eee140a0d2e57

Sliding preserves the upstream rule of merging each tile once per move. The
portable model stores tile exponents in individual bytes so 32768 tiles can
merge into 65536 and beyond without the old nibble representation overflowing.
The original MIT row implementation remains as a regression reference; tests
compare every row below its representation limit in both directions.
The full upstream license is included in assets/LICENSE-2048.txt.

Spawning uses fresh host-provided randomness, uniform selection among empty
cells and the pinned command-line application's 10% probability of a four.
Its initial two occupies the first row's second cell, followed by a random tile.
The graphical UI, merge scoring, best score, 2048 win/continue and game-over
states, restart, and 128-step undo are GamePKG integration features. The board,
score, best, continue flag and undo history persist in the game's scoped save.
Unknown or damaged saves are preserved until the player explicitly starts anew.
Host history reports scores up to its u32 limit; the board UI and save retain u64
scores. Exponents are bounded at 63 for checked integer arithmetic.

Controls: arrows/WASD move, Z/controller B undoes, Enter/Start begins a new game,
and C/controller A continues after winning. Held input does not repeat moves.
A restart keeps the best score. Undo restores the previous board and score;
it keeps the best score and does not consume random values.

No random crate, OS device or clock is exposed to the game. Solver assistance
and autoplay from the original application remain unfinished. Building this
source requires the updated GamePKG source host with `gamepkg::random_u64()`;
previously published host binaries may lack that API. GamePKG's integration code
uses Apache-2.0 terms; original upstream code keeps its MIT notice.

Thumbnail artwork credits, source links, modifications and license terms are
included in NOTICE.
