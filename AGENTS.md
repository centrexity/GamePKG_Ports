# Port task instructions

Keep going until the requested task is complete. Read WORKSPACE.md before changing a port. Work on one port per task; preserve unrelated work and existing tests. This is the canonical ports repository. Do not edit the app's pinned ports submodule as if it were this checkout.

Game code must have no Cargo dependencies and must use GamePKG-owned interfaces for system services, controllers, windows/graphics, audio, files and networking. Existing native direct calls are required migration gaps. Do not exempt entire engines, weaken checks or claim renamed wrappers alone provide security. Shared service implementation belongs in the GamePKG app; request/coordinate interface work there as needed.

Keep supplied ROMs in place and out of Git. Never publish private assets, evidence, keys or credentials. Preserve native generated Rust and avoid introducing Python as a user requirement. Run relevant checks locally. Do not push or release unless requested.
