# Ports workspace

This directory is the independent GamePKG_Ports Git repository. Start individual port tasks here. The local `workspace/individual-ports` branch begins at reviewed commit `470f36f`; it includes local integration work beyond the last published main. No changes have been pushed for this workspace move.

GamePKG application: `/media/user/660p1/c-site1/GamePKG/app`.
Ports repository: `/media/user/660p1/c-site1/GamePKG Ports`.

## Ownership

The top-level game folders contain package manifests, launcher/source recipes, thumbnails and notices. `host-integration/engines/<port>` contains tracked, asset-free integration source copied from the active app so each port task has its current patches, bridges and tests. `host-integration/snapshot.json` records provenance and file hashes. This is a migration starting point, not a second automatically synchronized build source. Existing app builds still consume their pinned `engines/<port>` copies. Coordinate any change that needs new shared interfaces with the app task before updating its pinned integration.

GamePKG owns generic input/controller, window/graphics, audio, filesystem, networking, permission enforcement, preparation/toolchain and process-launch services. Shared interfaces remain in the app under `include/`, `wit/`, `engines/native-input/` and `src/`. Game-specific translation, compatibility, generated game source, patches and package metadata belong here. Keep the generic host implementation in GamePKG.

## Required security migration

Ordinary game sources must have no Cargo dependencies. All game code, including native ports, must use GamePKG-owned interfaces for system services, files, networking, controllers, windows, graphics and audio. Native integrations currently retain direct platform/library calls; these are unfinished required work, not approved exceptions. Do not call a whole game engine a trusted host to bypass this rule. Move platform implementations behind audited host interfaces with runtime permissions and confinement; renamed functions and source checks alone do not enforce security.

## Individual port handoffs

- **Halo:** `halo/`, `HALO.md` and `host-integration/engines/halo/`. Shared-input, read-only ISO handles, settings and intro decoder checks exist. Normal Windows preparation/player launch and full service mediation remain unfinished. Private movie checks used dummy audio and do not establish audible output, campaign correctness or 60 FPS.
- **Burnout / Takedawn:** `takedawn/` and `host-integration/engines/takedawn/`. Preserve native generated Rust, image-in-place access, visuals and existing launch defaults. Rejected vertex-ring experiments must not be promoted. Complete host service migration and requalify controller input, frame pacing and audio on both devices.
- **Cube 2, Doom, Zelda 3:** matching game folders and `host-integration/engines/<port>/`. Migrate direct engine services without losing current input, rendering, ROM selection or native launch behavior.
- **2048 and Arkanoo:** corresponding top-level folders; preserve the dependency-free package policy.

## Local evidence and preserved files

The original verification evidence remains under `/media/user/660p1/c-site1/GamePKG/.verification`; it is private and is not part of this repository. Halo evidence includes `halo-windows-20261113`, `halo-input-20261113`, `halo-movies-windows-20261114`, `halo-settings-20261114`, `halo-contained-20261114` and `halo-windows-prepared-20261114`. The latter contains a draft Windows recipe, not a finished player release. Burnout evidence includes `app-d3d11-session-20261114`, `burnout-return-menu-20261114`, `burnout-cube-profile-20261114` and the rejected `burnout-vs-ring-20261114` experiment.

The old flattened ports folder is preserved in ignored `.local/legacy-workspace-ports`. Old checkout paths are compatibility links. Do not publish private evidence, credentials, keys, ROMs, extracted assets, saves or local tool bundles. User ROMs and remote test installations were not moved or modified.

Local catalog validation: `python3 tools/check_catalog.py`. Python is maintainer tooling here; it must not become an end-user requirement. Use the app's checks when changing host interfaces. Build and test locally; publish only when requested.
