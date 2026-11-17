# Checked RTL imports

These are dependency-free game-side adapters over the existing checked guest
memory service. They add no host API, raw pointer, crate, file access or title
admission exception. They do not run the full game or enable its native profile.

## Import contracts

All argument words and the return address are read before dispatch. Successful
calls change only EAX and ESP (return address plus the listed argument bytes);
other serialized CPU state is preserved. Guest-memory faults leave CPU state
unchanged; accepted guest field writes remain visible and owned.

| Ordinal | Import | Argument bytes | Result |
| --- | --- | ---: | --- |
| 269 | RtlCompareMemoryUlong | 12 | Matching **bytes**, comparing complete little-endian words only; null base returns zero. |
| 279 | RtlEqualString | 12 | BOOLEAN equality; complete 8-byte descriptors, equal byte lengths, nonnull buffers, C-locale byte folding when the low byte of the flag is nonzero, and strncmp termination at embedded NUL. |
| 289 | RtlInitAnsiString | 8 | Zero EAX; NUL scan, length capped at 65534, maximum length wrapping by one, original buffer address. |
| 290 | RtlInitUnicodeString | 8 | Zero EAX; little-endian u16 NUL scan, byte length capped at 65534, maximum length wrapping by two, original buffer address. |
| 301 | RtlNtStatusToDosError | 4 | Exact pinned bridge status mapping; unknown values return 317. |

Both string descriptors are read before deciding whether a buffer is null;
MaximumLength is ignored only after reading the full header. Zero-length strings
still require nonnull buffers but never dereference those buffers. Constructors
with a null destination do nothing; a null source writes a zero header. A nonnull
source is scanned through its terminator before length clamping. Header writes
occur in Length, MaximumLength, Buffer order. A later failure retains accepted
earlier stores rather than inventing an atomic rollback. This intentionally
preserves the pinned bridge, including Unicode maximum-length wrap to zero.
Case folding is the original default C locale, not Unicode case conversion.

`Memory::memory_read_chunk(address, unit, output)` accepts scalar units 1/2/4,
a nonempty multiple-of-unit output up to 4096 bytes, and validates the live root.
It reads whole units within the actual logical span and host backing page. If
a required unit crosses a boundary, the ordinary exact segmented read decides
whether it is accessible. This is no memory lease: protection, revocation and
other actual host read failures propagate immediately without smaller retries.

## Qualification

`check.py --app APP --app-lib RLIB --wine --evidence NEW_DIRECTORY` checks exact
source-kit member/body hashes, real ordinal dispatch and argument-byte tables.
Its maintainer C oracle preserves the pinned six original bodies and license.
No C, Python, oracle or fault shim is packaged as game code.

Four native target builds compare 2872 exact vector bytes. Linux32/64 run both
native components and signed confined children; Windows32/64 component execution
uses Wine, while their signed artifacts are build/audit-only. Each target runs
five one-shot actual SDK read errors. Fourteen broken native copies must fail,
including endian/word-count/BOOLEAN/NUL/clamp errors, guard-page/span mistakes,
read-error retries and invented header rollback. Two broken copies also pass
ordinary source admission and all-object auditing before failing in the actual
confined Linux child. Every optimized game object is audited before trusted
linking, with LTO disabled and exact SDK/compiler/source/object receipts.

The authenticated KIC4 image suite `../service-kernel-imports/check_sync.py`
adds mode 81, calling all five imports through the actual prepared import plan
and asserting CPU, caller, guest output and the exact diagnostic vector. Existing
startup ownership, joins, failure-prefix cleanup and seven required-failing
startup controls remain active. Private image verification reads the supplied
ISO in place and keeps its evidence outside Git. Four imports occur in the real
main table; ordinal 290 is additionally tested by the synthetic image.

These checks establish helper/import/startup behavior, not original title
handler/loop execution, rendered gameplay, physical controls, audible sound or
measured frame pacing. Mixed NT waits/close, file/thread/APC integration and the
remaining renderer/media/build/launcher migration remain required.
