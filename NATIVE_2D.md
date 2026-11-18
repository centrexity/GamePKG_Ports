# Testing the native 2048 and Arkanoo sources

These source trees need a GamePKG Linux x86-64 build with the native 2D tick
profile. The profile provides the generated GameState and text helpers and the
shared input, window, audio, and file picker used by these ports. Game packages
remain dependency-free. The normal Build and Play actions select the audited
native build. Wasm remains an explicit target.

Use the GamePKG Pack command and your local test creator key to package either
source directory for the compatibility build. Use a separate test library if
the same game ID is already installed under a different creator key.

The published catalog still points to the currently released, signed packages.
This branch contains source changes for testing. It requires the corresponding
GamePKG compatibility build before it can replace the release sources on main.
