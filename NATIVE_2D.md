# Testing the native 2048 and Arkanoo sources

These source trees need a GamePKG Linux x86-64 build with the native 2D tick
profile. Use [GamePKG 14df78a](https://github.com/centrexity/GamePKG/commit/14df78a7ad2ac0b5872539bcbcab4e0b9121eb4c)
or a later build that includes the profile. The profile provides the generated GameState and text helpers and the
shared input, window, audio, and file picker used by these ports. Game packages
remain dependency-free. The normal Build and Play actions select the audited
native build. Wasm remains an explicit target.

Use the GamePKG Pack command and your local test creator key to package either
source directory for the compatibility build. Use a separate test library if
the same game ID is already installed under a different creator key.

The published catalog still points to the currently released, signed packages.
Build the sources with the corresponding GamePKG compatibility build. The next
signed release can publish new package versions for ordinary catalog updates.
