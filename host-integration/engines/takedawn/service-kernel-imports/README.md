# Checked complete import preparation

`imports.rs` plans the complete original main run and 16-word lookbehind using
ordinary dependency-free game source, the initialized kernel-data component and
the finite allocator bridges. `image_imports.rs` binds that plan to the reviewed
image/CPU setup. No native function pointer, OS protection call, success stub or
engine exemption is introduced. **The title remains gated:** the 23 migrated functions
(nine allocators, six SHA/RC4/HMAC, NV settings, four clocks, HAL vector query and
two dispatcher initializers) and 24 data exports are available without context.
KeConnectInterrupt is additionally available only through the bound KPL4/KIC1
context; see [the current contract](../service-kernel-context/README.md). See
`../service-kernel-dispatcher/README.md` for the initializer contracts. A complete table
containing any other function fails Unsupported before publication.

Requires shared SDK local op 17, available from app `8e741b5`: full digest
`22be2884a8921bfbcacb3b8f3f1fe8da111840f0bb3b600269a21d36221cc0be`, composite
native recipe `d8f147d4eed6560b`. Older services may return Unsupported; never
guess page protections or fall back to system APIs. No host extension beyond
that query is required by this component.

## Exact contracts

`Imports::imports_scan(&Memory, table: u32, &kernel_data::Data, remap: &[u16])`
requires a live root, no existing kernel map, table at/above `0x10000`, a complete
bounded run and at most 65,536 copied remap entries. It derives the count; callers
cannot supply a shorter one. A nonzero run must terminate within 378 entries,
including a readable zero after entry 378 when that edge is reached. Oversized
or unterminated runs are Invalid rather than clamped. The original setter keeps
its initial 378 count when passed zero: a leading zero therefore causes all 378
main slots to be inspected, not an empty successful plan. This is a fresh
bootstrap model, not the original globals reused across arbitrary repeated init.
All extents, including lookbehind and required terminator, must fit main RAM.

The plan snapshots the 64 preceding bytes plus the entire effective main range
and terminator where applicable. It detects count/snapshot disagreement as Busy.
It preserves ordinary non-marker bytes. Main markers first apply the original
nonzero u16 remap entry if raw ordinal is within its slice; otherwise identity.
Canonical data ordinals point to initialized data addresses. Other ordinals must
be implemented by `kernel_supported`; they receive `0xFE000000 + slot * 4`.
Unknown functions are Unsupported (-5), never genuine table absence/EAX-zero
fallback. Data/table overlap is Invalid.

The 16 preceding words preserve the original asymmetry: no remap or data lookup,
raw function ordinals only. Distance `i` uses slot `main_count + i`; slot
`main_count` stays empty. Unsupported functions/data in that region fail; a slot
at/above 378 fails Invalid before any patch. The original may stop scanning at
capacity and leave markers unresolved; this strengthened path refuses to hide
them. It neither truncates the title table nor promotes unmigrated handlers.
Unaligned guest table addresses use explicit LE byte transfers, not host loads.

`imports_layout() -> [u32; 3]` copies `[table, main_count, routing_slot_count]`.
The plan borrows its root, not a native pointer or exclusive HeapClient. The
port-owned `Memory::memory_page_backing(page) -> [u64; 2]` requires that same
live record, a 4 KiB guest-aligned page wholly inside one attached span and a
4 KiB backing-aligned offset. It copies `[opaque window ID, offset]` only;
GamePKG still enforces every operation/lifetime. No view/permission is created.

`imports_image(&Memory, &xbe::Image, &Data, remap)` is the production coordinator
adapter. It checks the loaded image's executable bounds against the live call
record, requires data initialized at that image's derived bootstrap kernel-data
region, derives the table from the authenticated image and ensures the complete
effective main extent lies within its image bounds. These checks compose with
`image_plan` digest verification and `image_load`; they are not a replacement
for authenticating the source kit or serializing startup. Use this adapter in
the real coordinator instead of selecting a convenient table address.

`imports_publish(&mut Memory)` rechecks root, absence of an existing map, exact
page backing IDs/offsets/states and original bytes. Changed bytes or protection
snapshots are Busy; wrong root/backing is Denied. Physical allocator functions
also require the initialized shared Contiguous owner; its observation restores
the exclusive client even on error. Missing Contiguous stays Unsupported.
Record quota is checked before temporary permissions. At most two pages are
involved; original RO pages temporarily become RW, the combined copied patch is
written through checked RAM, then changed pages restore in reverse order before
installing the map. Already RW pages are not changed. A translation-level RO
view still denies the write, and temporary host permissions are restored.

Every changed page restores its own original state. The original C uses one
first-page protection value for a bulk restore and can homogenize mixed pages;
this implementation deliberately retains each page's prior rights. C comparisons
qualify exact RAM/slot effects; mixed-page restoration is independently checked
against the saved states, not claimed byte-for-byte reproduction of that bulk
protection behavior. No new read/write authority or writable fallback remains.

Startup must serialize writers: snapshot comparison, protection changes, patch,
restoration and record install are separate runtime calls, not one transaction
or lease. Restoration is attempted on all changed pages even after failure;
restoration errors take precedence. Failed publication releases its new record.
RAM might already be patched if restoration/root installation fails. Abort and
end the private runtime on such errors; do not replay or claim rollback. A
successful map stays installed until explicit coordinated `kernel_clear`.
Other callers share its ID through the existing four-word context, never a
response queue/client sequence. Do not clear it while workers still use it.

## Private KPL3 record

The existing 4 KiB/64-record/64 KiB quotas and maximum 1,540-byte map stay intact:

| Offset | LE field |
| --- | --- |
| 0 | magic u32 `0x334C504B` |
| 4 | routing slot count u32, 1..378 |
| 8 | associated root call-record ID u64 |
| 16 | main table guest address u32 |
| 20 | main count u32, 1..routing count |
| 24 | routing count canonical ordinal u32 values; zero means empty |
| `24 + 4 * routing_count` | shared refusal counter u32, 0..8 |

Total length is `28 + 4 * routing_count`. With extras, routing count is
`main_count + 2 .. min(main_count + 17, 378)`, the gap is zero and the final
extra slot is nonzero. Extra slots contain only zero or implemented functions.
Live root, lengths, ranges, layout and counter are checked on every invocation.
Main data ordinals are stored for exact classification but are not callable.
Old KPL2 remains strict about its reserved-zero word and aligned table, and
keeps all existing behavior. CPU record (244 bytes), call record (280 bytes),
context IDs, actor protocol and all source/object/runtime limits are unchanged.

## Validation

`reference_inputs.json` pins the source kit and exact original full bridge init,
count/remap setters, argument-size function, allocator bridge, lookup membership
and original image-loader count scan. The C oracle also uses the previously
pinned original kernel-data, crypto and Heap bodies. Only known migrated
functions appear in positive vectors. Other C function addresses are used for
classification only and are never invoked; actual routed allocations use the
original ExAllocatePool body. No oracle code is linked into admitted objects.
The bounded original C int/u32 loop's signed-compare warning is scoped only in
that maintainer oracle; the pinned body/types remain unchanged. Existing native
conformance lint allowances cover generated names/parentheses only. They do not
relax game-source policy or object auditing.

```sh
python3 host-integration/engines/takedawn/service-kernel-imports/check.py \
  --app /path/to/GamePKG/app --app-lib /path/to/libgamepkg.rlib \
  --call-targets /path/to/verified-targets --wine
python3 host-integration/engines/takedawn/service-kernel-imports/check_image.py \
  --app /path/to/GamePKG/app --app-lib /path/to/libgamepkg.rlib \
  --call-targets /path/to/verified-targets --iso /path/to/your.iso
```

Native Linux32/64 and Windows32/64-under-Wine each compare 16 complete setups,
all 378 routing cells/argument sizes/classifications, every 64 MiB RAM byte and
allocator counters. Inputs cover all supported function/data ordinals, remap
identity/zero/high indices, raw lookbehind, zero-count fallback, maximum counts,
gaps, ordinary bytes, lowest/unaligned/cross-page tables and mixed protections.
Actual traced/untraced calls, all target-byte offsets, data-call rejection and
untouched CPU fields are checked through the real finite router. Controls cover
oversized/unterminated runs, unsupported/remapped/lookbehind imports, capacity,
stale bytes/protections/backings, wrong/revoked root, late decommit, missing
Contiguous, translation-RO denial with restoration, released failure records,
unchanged record quota and corrupt KPL3 fields/gaps/counter.

Both fixtures pass complete source admission before bindings, independent game
optimization and every-object audits before trusted linking on four targets.
Actual confined Linux32/64 checks compare the original import/metadata/routing
witnesses and strict controls. Full RAM comparison belongs to the separate
native oracle runs, not the confined pipe witnesses.

The image fixture composes in-place XDVDFS, authenticated image loading,
CPU/TIB/TLS setup, the shared allocator, correct data region and import planning.
Retail/debug synthetic images reach real native allocation through the patched
table; unsupported imports, wrong code bounds and wrong data region reject
without table publication. Synthetic mode 6 also routes SHA init/update/final
through the complete image/import/Call pipeline and verifies the `abc` digest.
The authorized private ISO remains read-only and
unextracted. Its main table currently contains **115 unmigrated function exports**;
it rejects Unsupported without changing the table. Numeric inventory is private
test evidence, not a claim those functions are implemented. No title entry runs.

The existing full partial generated-candidate 38-function/4,864-vector matrix,
old pool/allocator wrappers and image/CPU regressions remain independent checks.
Python/C are maintainer-only. Tests are silent: no game window, real graphics,
physical controls or audio. Windows cross-build/Wine is not AppContainer proof.
The installed shared SDK is unchanged by these port-only helpers. Complete the
remaining kernel/guest-thread/IRQ/device and generated/manual/media/renderer
paths, periodic time/status integration and native entry before enabling title
build/play/cache/export or claiming gameplay/frame-pacing qualification.


The RC4/HMAC extension also exercises image mode 7 through the actual finite
router and requires the current clearing SDK; see
`../service-kernel-crypto/README.md` for exact argument/alias/error contracts and
app pin `2c646c0ec78cb39b1251ccd7ba8662da5fc62ffa`. No root/page/record quota
or source/object policy was relaxed.


The shared root services/NV extension adds original export 24 with configured
video/audio flags and one shared offset. See `../service-kernel-settings/README.md`
for the exact 288-byte private root codec, 64-byte record, unchanged four-ID
context, readiness/lifetime/error contracts and C/confinement controls. CPU
records stay 244 bytes and all quotas stay intact. The private main gap is now
114; time exports and full gameplay remain gated.

Clock consumer checkpoint `b70e07e` adds 125..128 with valid shared configuration
for 125/126/128 and independent frequency support for 127. Image mode 10 verifies
all four through authenticated synthetic loading, full import publication and
the actual finite Call router, with real clock bounds and full CPU completion.
The private ISO remains in place and rejects 111 remaining main function exports
before publication/title entry. See `../service-kernel-clock/README.md` for the
exact arithmetic, null, partial-effect and validation contracts. No complete
title, production recipe or admission exception is enabled by this change.

The current dispatcher extension registers only complete 44/107/109 behavior.
107/109 require the configured KSV3 owner before import publication; 44 remains
independent. Actual private in-place image checks report 108 unresolved main
functions and continue to reject before patching/title entry. No timer, IRQ,
DPC-execution or renderer handler is made available by object initialization.


## KPL4 context-dependent calls

`imports_scan_with_context` and `imports_image_with_context` bind a live checked
KIC1 registry context without changing complete-table/image validation. The new
map starts slots at 32 and is at most 1548 bytes; KPL2/KPL3 retain their layouts.
Ordinal 98 requires this binding; old context-free entry points still reject it.
Current CPU/root sizes are 316/288 bytes and the four-ID context is unchanged.
See `../service-kernel-context/README.md` for exact formats, APIs and scope.
