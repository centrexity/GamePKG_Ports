GamePKG adaptation of Adrien Ball’s 2048-rs.
Upstream: https://github.com/adrienball/2048-rs
Revision: 9e972c43e51b153f66b31a08c37eee140a0d2e57

Row sliding and merge logic is adapted from src/utils.rs under the MIT license.
The terminal UI, solver, lookup-table macros and dependencies are replaced with
direct GamePKG drawing and input. Tiles cap at 32768 to fit the upstream nibble representation.
Full upstream license is included in assets/LICENSE-2048.txt.

Spawning uses fresh host-provided randomness, uniform selection among empty
cells and the pinned command-line application's 10% probability of a four.
Its initial two occupies the first row's second cell, followed by a random tile.
No random crate, OS device or clock is exposed to the game. Scoring, end-state
UI, restart/undo, persistent boards and solver support are still unfinished.
Building this source requires the updated GamePKG source host with
`gamepkg::random_u64()`; previously published host binaries may lack that API.

Thumbnail artwork credits, source links, modifications and license terms are included in NOTICE.
