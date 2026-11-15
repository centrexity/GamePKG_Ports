# Burnout renderer contract review — 2026-11-14

For the current SDK and executed component results, read
[SERVICE_STATUS.md](../SERVICE_STATUS.md). The interface tables below are the
historical source review; several requested facilities now exist in the app.

**Source review, not a completed renderer migration or a gameplay inventory.**
App inputs: `docs/TAKEDAWN_SERVICE_HANDOFF.md`, `docs/PORT_SERVICES.md`,
`src/port_host/graphics.rs` and `graphics/retained.rs`. No app changes or profile
admission changes are authorized by this review. The broader app regression
result is still pending in the coordinating task; its reported offscreen RTX
3090 tests are not Burnout gameplay qualification.

Port input: recipe 0.1.16, source kit SHA-256
`6b37d24c0984c686c7ed6fd73cd3a5b252b5631ceefa2798e5b7a6e0adff8dc4`,
plus the canonical `media-bridge` adaptations. `source_inventory.json` records
17 archive source hashes, seven port source hashes and 11 declared enum families.
The inventory contains no game assets, vertex-program bytes or private traces.
Regenerate only after deliberate review with
`python3 host-integration/engines/takedawn/service-renderer/review_contract.py --write`;
omit `--write` to check it. Python is maintainer tooling, never a player requirement.
Matching hashes establishes source identity, not service admission or correctness.

## What the new contracts resolve

| Previously requested facility | Current contract | Port adaptation still needed |
| --- | --- | --- |
| Retained surfaces and ordered draws | 106/108/110: opaque targets, retained RGBA8/depth, ordered passes and rectangular clears | Translate guest surface identities, clear masks and coordinate scaling; do not reset preserved attachments between batches. |
| Feedback/color copy | 107: overlap-safe resolved color copy, including MSAA destination | Snapshot current output before self-sampling. This operation does not copy into ordinary texture/cubemap layers. |
| Immutable display | 109: budgeted snapshot, one pending frame | Retry Busy without losing/reordering accepted game frames; retain the original presentation crop/aspect/scale behavior. |
| Real visibility | 111/115/113: bounded asynchronous whole-pass sample counts | Split title query spans at pass/clear boundaries, accumulate completed segments, and supersede stale title report slots. No always-visible substitution. |
| Save publication | 14: flush/atomic replace; -6 means committed but directory durability failed | Do not retry a consumed source after -6. Windows directory durability is Unsupported before mutation; choose and document flags for each platform. |
| Time/wakeup | 60–62: session monotonic time and worker-only bounded wait | Use `channel_with_timing` with the same Session clock, absolute deadlines and generation-based wakeup. Host events wake waits. |
| Audited staging | CheckedSources/AuditedStage and pipeline identities | Still needs an actual reviewed Takedawn builder and private transport launcher; the helpers alone do not permit build/play. |

## Vertex instruction and interface inventory

These are the pinned parser/emitter's accepted vocabulary, **not evidence that
every instruction occurs in Burnout**. Numeric values are in the JSON inventory.

| Family | Enumerated values / limits | Required semantics |
| --- | --- | --- |
| MAC, 0–13 | NOP, MOV, MUL, ADD, MAD, DP3, DPH, DP4, DST, MIN, MAX, SLT, SGE, ARL | ADD uses A+C; DPH adds B.w; ARL emitter uses floor(A.x + 0.001). Preserve reference behavior before considering numerical changes. |
| ILU, 0–7 | NOP, MOV, RCP, RCC, RSQ, EXP, LOG, LIT | MOV is vector; other operations have component-specific results. EXP and LOG return four-component helper results, not just replicated exp/log. RCC bounds/sign, LOG(0), LIT exponent limits and reciprocal edge values need differential tests. |
| Program/register bounds | At most 136 instruction slots, 192 constant vec4s, 16 input vec4s, R0–R11 plus R12 aliasing oPos | 128-bit instruction words; MAC and ILU read pre-write values in the same slot; both results are computed before writes. A busy MAC redirects the ILU temp destination to R1. |
| Addressing | Temp, input, constant and zero source mux; negation, four-component swizzles, write masks, relative a0 constant reads, final flag | Use `kernel/nv2a_vsh_fields.h` and actual parser code. The bit-layout prose in the parse header is stale; word 0 carries no program fields. Relative constant reads clamp to 0–191 in the GLSL emitter. |
| Outputs | oPos, oD0/oD1, oFog, oPts, oB0/oB1, oT0–oT3 | Unwritten outputs start (0,0,0,1); color outputs saturate. Back-color outputs are decoded but not exposed by the current GLSL VS/FS varying interface. Do not claim hardware completeness. |
| Input formats | D3DCOLOR, normalized signed-short S1, FLOAT, normalized UBYTE, unnormalized S32K, packed signed 11:11:10 CMP | Existing gather converts to interleaved float4 attributes; at most 16 attributes, stride 256. Preserve endian/normalization/default components, and ascending used-register attribute order. |
| Varyings | diffuse/specular vec4, four texcoord vec4, scalar fog | Current GLSL locations 0–6; perspective interpolation and fragment alpha discard must remain equivalent. |
| Fixed-function path | `s_vs_pass_glsl`, `s_fs_basic_glsl` in `nv2a_pb_vulkan.c` | Pretransformed position, diffuse modulation/texture/pass-through and the same quantized alpha test; a program-only template would omit this path. |

The existing three uniform blocks are vertex constants (3,072 bytes), viewport/
fog/specular (80 bytes), and pixel constants (the 416-byte shared block plus LOD
bias/bump parameters and optional HDR tail). A port shader can repack these into
the service's single binding-zero block with explicit offsets; no extra uniform
binding API is inherently required. A bounded interpreter's instruction data
must also fit the 64 KiB limit. This is a feasibility observation, not a prepared
or approved interpreter.

The canonical `adapt_vertex_perspective.cmake` is part of the baseline. It
reconstructs signed near-zero W using the RCC ceiling reciprocal instead of
substituting W=1. Preserve the near-plane road correction, D3D Z in 0..W,
far-plane rounding guard, fog/specular processing, coordinate orientation and
polygon-offset handling. Do not translate the unadapted archive emitter alone.
Reject or explicitly qualify unsupported constant-write instructions: the old
parser ignores ORB=0 constant writes and maps invalid MAC opcodes to NOP. Those
legacy limitations are not permission to admit arbitrary new IR.

## Combiner and texture inventory

Sources: `d3d8_combiners_parse.h/.c`, `d3d8_combiners_glsl.c`,
`d3d8_glsl.h`, and the CPU evaluator `kernel/nv2a_combiner.c`.

| Family | Required vocabulary / behavior |
| --- | --- |
| Registers | ZERO, per-stage C0/C1, FOG, V0/V1, T0–T3, R0/R1, final-only V1R0_SUM and EF_PROD; reserved register IDs 6/7 need explicit validation. |
| Stage bounds | 0–8 general stages, four texture stages, seven final-combiner inputs A–G; independent RGB and alpha portions. |
| Input mappings, 0–7 | UNSIGNED_IDENTITY, UNSIGNED_INVERT, EXPAND_NORMAL, EXPAND_NEGATE, HALFBIAS_NORMAL, HALFBIAS_NEGATE, SIGNED_IDENTITY, SIGNED_NEGATE. Implement emitted math; header prose claiming universal [0,1] clamping is not an adequate oracle. |
| Output mappings | Identity (0), bias (1), shift-left 1 (2), shift-left 1 with bias (3), shift-left 2 (4), shift-right 1 (6). The old helper treats reserved 5 as identity; a new validator needs an explicit policy. |
| Stage operations | Component products, dot3 AB/CD, sum or R0.a-selected mux (MSB or quantized LSB), AB/CD/sum destinations, blue-to-alpha, per-stage/unique constants, simultaneous input evaluation before destination writes. |
| Final combiner | Evaluate E/F before EF and V1+R0 special registers, then A–D/G; RGB formula A*B+(1-A)*C+D; G supplies alpha. Actual GLSL emitter uses 0x40 to complement V1 and 0x20 to complement R0; the header's final_flags comment reverses them. Verify against evaluator fixtures rather than copying that comment. |
| Alpha/fog | All eight compare functions on rounded 8-bit alpha/reference; actual discard must affect visibility counts. FOG may use interpolated vertex fog or the constant fog color. |
| Texture modes, 0–7 | NONE (0,0,0,1), projective 2D, 3D, cubemap, clamped coordinate pass-through, per-component clipplane discard, bump-env and bump-env-luminance with predecessor selection/matrix/luminance parameters. |
| Texture modifiers | Signed channels, alpha-only white RGB, per-stage coordinate scale, projective q division, per-stage LOD bias, mip chain, bump-source selection. |
| Samplers | Independently selected min/mag/mip filtering, LOD min/max, anisotropy and wrap/mirror/clamp. Current service has one bit selecting all three filters together, no LOD range or anisotropy descriptor. Shader bias alone does not replace these settings. |

The translator can emit sampler3D, but the reviewed Vulkan binding path does not
establish a working volume-texture upload/provider. Mark 3D as a declared mode
requiring actual-use evidence and fidelity review, not as a proven working
feature or an unconditional volume API demand. Similarly, modes outside 0–7
fall through to a 2D approximation in the old emitter; do not infer full NV2A
coverage from a mode hash.

Ordinary decoded textures can remain port-side RGBA uploads, including palettes,
compressed/YUV decoding and swizzles. BGRA-to-RGBA conversion must happen once,
without changing color space. More urgent is **rendered cubemap assembly**:
`cube_bind` copies drawn targets to individual cube faces on the GPU and retains
other faces/mips from decoded guest memory. Operations 102/103 allow CPU cube
uploads, while 107 copies only target-to-target. Neither offers target-to-cube-
face/mip copy. Agree a bounded opaque destination layer/mip copy contract or an
equivalent verified cube-sampling representation. Do not replace reflections
with stale CPU bytes or introduce unbudgeted per-frame readback.

## Render state, attachments and copy requirements

These are source paths that must be represented or explicitly ruled out with
scene evidence, not a request to enable every optional legacy setting at once.

| State/facility | Current renderer | Service status / coordination |
| --- | --- | --- |
| Primitive topology | Points, lines and triangles after strip/fan/quad expansion (`nv2a_prim_to_list`) | Indexed service is triangle-list only. Need point/line evidence and either reviewed expansion preserving size/coverage or explicit pipeline topology support. |
| Blend | Zero/one, source/dest color/alpha and inverses, source-alpha-saturate, constant color/alpha and inverses; add/subtract/reverse/min/max | Installed blend descriptor covers many factors/ops, but there is no per-draw blend constant. Preserve independent color/alpha factor conversion. |
| Color masks | All RGBA write-mask combinations via `color_keep` | Pipeline hardcodes ColorWrites::ALL. Add verified masks/state variants; blending cannot substitute for masked writes in general. |
| Depth | All eight comparisons, write enable, constant and slope-scaled bias | Compare/write can be installed variants; bias is currently defaulted and not selectable. Preserve scaled slope bias and the signed-W correction. |
| Winding/cull | CW/CCW; none/front/back/front-and-back | Cull descriptor exists, front winding is fixed by default; front-and-back can skip drawing in port logic. Other variants need explicit representation. |
| Stencil | Eight compares; keep/zero/replace/increment/decrement saturating and wrapping/invert; read/write masks/reference; independent depth/stencil clears | Depth32Float has no stencil. Add a bounded stencil-capable attachment/state path before qualifying scenes using it. This is not merely a shader variation. |
| Depth identity | `depth_target` keys depth/stencil by guest zeta address and dimensions independently of color target | Service target owns its own depth. Retaining depth per color target is insufficient when multiple color targets share a zeta surface. Agree separately owned attachable depth/stencil IDs or equivalent tested identity-preserving behavior. |
| Color format | SDR Vulkan BGRA8; optional HDR RGBA16F, HDR10 output and tone mapping | RGBA8 can represent SDR with explicit channel conversion. HDR needs additional verified formats/programs if retained; do not claim HDR qualification with RGBA8. |
| Guest depth format | Z16 or Z24S8 interpretation; host prefers D32FloatS8, falls back to D24S8 | Float depth can hold normalized Z16 values, but stencil/precision behavior still needs tests. GPU host depth is not raw guest-format bytes. |
| Depth copy/resolve | New API does color only | Source review found independent depth reuse, not a demonstrated requirement for arbitrary depth copying. Prioritize preserving depth identity; request copy/resolve only with a specific use and semantics. Do not declare it unnecessary globally. |
| Surface ownership/readback | `rt_sync`, stale-target detection and CPU guest-memory aliases | Decide how GPU-written color becomes visible to CPU title reads without raw platform calls. A bounded readback/fence API may be required; color target copies do not satisfy a CPU read. Keep real dump/debug readbacks distinct from runtime coherence. |
| Visibility | Queries span draws and split around clears/pass boundaries, then sum segments into a title report | Whole-pass 115 can supply segments when command batching aligns boundaries. Preserve sample counts and report supersession; fit the 64-query/64-in-flight-job limits or provide backpressure. Do not assume one query equals one title frame. |

The legacy Vulkan pipeline cache has capacity 8,192, while the service currently
accepts at most 256 installed pipelines. Cache capacity is not observed active
variant count. A finite prepared catalog must prove completeness and fit the
service bound, or coordinate bounded pipeline registration. Shader hashes alone
cannot justify a larger limit or establish valid semantics.

## Complete postprocessing review scope

Defaults stay windowed, 16:9, render scale 2, SMAA HIGH, 10% sharpening, automatic
LOD, widescreen HUD correction and menu/HUD scale 2. No default was changed here.

| Variant | Passes / resources that must be reviewed |
| --- | --- |
| SMAA 1x HIGH (default) | Native-resolution luma edges -> blend weights -> neighborhood blend; threshold 0.1, 16 search, 8 diagonal, corner rounding 25. Clear edges/weights first because edge detection discards. |
| SMAA tables | Area 160x560 RG8 and search 64x16 R8. RGBA8 repacking may preserve sampled channels; demonstrate lookup-channel and filtering equivalence before avoiding new formats. |
| CAS (default 0.10) | Shared POST_CAS_BODY after AA, correct source pixel size and strength, opaque output alpha. Also qualify CAS alone and zero strength. |
| FXAA and AA-off | Shared POST_FXAA_BODY -> optional CAS; AA-off -> optional CAS; preserve runtime choice and switching/resource retirement, not just startup default. |
| D3D11 fused CAS/blit | GP_POST_CAS_BLIT uses a point-sampled R32G32 coordinate texture to select native pixel centers, then CAS. RGBA8 coordinates lose precision. Agree float/uint coordinate storage or an exactly equivalent signed shader calculation and compare pixels. |
| Crop/scale/present | Padded target margins, native crop, aspect and HUD correction, nearest/linear scaling and fullscreen resize; color conversion exactly once. Presentation 109 alone is not the legacy shader blit. |
| Optional stencil optimization | `private_edge_mask.h` conditionally restricts SMAA weight work. Preserve correctness with the ordinary reference passes first; no performance claim for a newly ported mask without measurements. |
| Optional HDR | HDR combiner helpers, RGBA16F scene and post variants, tone mapping/encoding, PQ overlay and pattern programs. Explicitly unqualified pending scope/format agreement; never silently clamp the HDR path to SDR. |

## Proposed next interface agreement and acceptance gates

Prefer a finite reviewed program set where complete, and assess a **bounded
signed vertex/combiner template** where a full static catalog cannot be proven.
This is a proposal, not an approved new runtime API. Put game translation,
validated instruction/state encoding and action mappings in the port. Keep
generic resource/state enforcement and verified shader preparation in GamePKG.
No arbitrary GLSL/WGSL/SPIR-V requests and no NV2A/engine-in-Backend exemption.

Before implementing a template, agree versioned explicit fields, register and
instruction bounds, allowed opcodes/modes, uniform layout, resource dimensions,
reserved-value behavior, independent depth identity, pipeline/state limits,
and unknown-feature failure. Differentially test the existing parser/evaluator
and adapted emitter on synthetic vectors, especially paired instruction writes,
relative constants, reciprocal edges, combiner mux/flags, signed/bump textures,
alpha-discard visibility and the near-plane correction. These tests establish
the interpreter's defined subset; original-hardware equivalence remains a
separate question.

Then compare GPU pixels for actual shader variants and ordered scenes: menu and
movie quads, countdown/road clipping, race/crash, car reflection cube faces,
fog/transparency/depth/stencil, save/result transitions, and all AA/sharpening
choices. Capture private bounded feature telemetry in actual supported scenes to
distinguish unused declarations from required variants; do not publish shader
microcode, textures, user saves or screenshots. Keep catalog/profile admission
blocked until migrated builder, launcher, services and gameplay qualification
are complete. No performance optimization or main merge is justified by this
source review alone.
