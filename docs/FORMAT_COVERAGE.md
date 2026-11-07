# Disk-format coverage matrix

This is the authoritative statement of *which on-disk formats and format
features* `acornfsextract` is known to handle and how each is tested. It is
generated/reported from `src/format_coverage_tests.rs` (report-only: the test
always passes and just prints the matrix, so CI surfaces regressions in format
breadth without failing the build on a coverage shortfall).

Ground truth is **real media authored by the official Acorn tools**, never
this project's own builder and not a second reader (the `filecore-extract`
reference tool is retired). A synthetic/self-generated image is only ever
trusted as a *carrier* or an *edge-case*; its expected values are derived from
an official-tool golden before they are baked into a test.

## Legend

| Status | Meaning |
|--------|---------|
| `real` | Exercised against a real (official-tool) disc image. |
| `real (runtime)` | Exercised against real media fetched at runtime (key-gated, never committed, no pinned hash/name). |
| `synthetic` | Exercised against an in-repo generated image, but cross-checked against an official golden. |
| `synthetic-only` | Exercised only against an in-repo generated image; no golden anchor yet. |
| `uncovered` | No test; risk of silent mis-parsing. |
| `refused` | Recognised but deliberately rejected (a supported-limitation), with an explicit test. |

## FileCore (ADFS) / FileCore structures

Columns: **map** (old/new) × **dir** (old/new/big) × **geometry** ×
**interleave** × **zones** × **fragmentation**.

| Cell | Map | Dir | Geometry | Interleave | Zones | Status | Fixture / test |
|------|-----|-----|----------|-----------|-------|--------|----------------|
| S | old | old | 160 KB, 40 trk × 1 side | sequential→interleaved | 1 | `synthetic-only` (golden pending) | `sml_geometry` unit 640; S/M reference (pending) |
| M | old | old | 320 KB, 80 trk × 1 side | sequential→interleaved | 1 | `synthetic-only` (golden pending) | `sml_geometry` unit 1280; S/M reference (pending) |
| L | old | old | 640 KB, 80 trk × 2 sides | sequential→interleaved | 1 | `real` | `data/adfs640L.adl.gz` |
| D | old | new | 800 KB, 80 trk × 2 sides, 1024 B sect | interleaved | 1 | `real` | `data/adfs800D.adf.gz` |
| E | new | new | 800 KB, 5 sect/trk | interleaved | 1 | `real` | `data/adfs800E.adf.gz` |
| F | new | new | 1.6 MB, 10 sect/trk | interleaved | 4 | `real` | `data/adfs1600F.adf.gz` |
| E+ | new | big | 800 KB | interleaved | 1 | `synthetic-only` (real media available via corpus scan) | `testutil` `big_dirs` |
| F+ | new | big | 1.6 MB | interleaved | 4 | `synthetic-only` (real floppies exist in corpora) | `testutil` `big_dirs` |
| G | new | big | hard disc | interleaved | n | `synthetic-only` (real media available via corpus scan) | `testutil` `big_dirs` |
| Old-map hard disc | old | old(small) | 256 B sect, drive-geometry | linear | 1 | `synthetic-only` (opt-in corpus scan validates real media at runtime) | `oldmap_hard_disc_tests` (synthetic) + `corpus_tests` — real media (a 1984 Acorn Winchester File Server) shows old/small `0x500` dirs at `0x200`, linear addressing, disc_size = image length, and an uncomputed (zero) directory check byte; the guide's `0x400`/`0x800` new-dir assumption is corrected |
| New-map sequential track order | new | new | any | sequential flag set | n | `refused` | synthetic refusal test |

### Fragmentation (new map)

| Feature | Status | Test |
|---------|--------|------|
| Single zone, file in 2 same-id fragments | `synthetic` | `testutil::fragmented_file_reassembles_in_order` |
| File in >2 fragments | `uncovered` | seeded random generator (pending) |
| Cross-zone fragment span (`pending_span`) | `uncovered` | seeded random generator (pending) |
| Sharing offset on a non-root object | `synthetic-only` | `build_new_map_disc`/big-dir paths |
| Fragment in a big directory | `synthetic-only` | `testutil` `big_dirs` |
| Free-chain exclusion interacting with real fragments | `synthetic` | `map_new` unit tests |
| Real-media fragmentation | `real (runtime)` | Arcology key-gated `hard_disc_fragmentation_against_arcology_corpus` (14/15 sampled real `.dd` hard discs had fragmented files) |

### Structural / metadata features

| Feature | Status |
|---------|--------|
| Boot block present (F format / hard disc) | `real` (`adfs1600F`) |
| Boot block absent (S/M/L/D, E) | `real` (L/D/E) |
| Disc-record zone-0 vs boot-block merge | `real` |
| ZoneCheck / cross-check verification | `real` |
| Directory check byte (A.2) | `real` |
| Old-map free-space map checksum | `real` |
| S/M/L attribute bits in name bytes | `synthetic` |
| DOSFS / RISC OS charset name translation | `synthetic` |
| `.inf` sidecar creation | `synthetic` |
| Bad-sector policies (null/skip/marker) + rescue map | `synthetic` |
| Broken-directory policies (fail/skip/recover) | `synthetic` |

## DFS (Disc Filing System)

Columns: **vendor** × **geometry** (SSD/DSD, 40/80 trk, sect/trk) × **catalogue features**.

| Cell | Vendor | Geometry | Status | Fixture / test |
|------|--------|----------|--------|----------------|
| Acorn DFS SSD | Acorn | single-sided | `real` | `data/apd01_ssd.ssd.gz` (`dfs_reference_media_tests`) |
| Acorn DFS DSD | Acorn | double-sided interleaved | `real` | `data/8bs0_dsd.dsd.gz` (`dfs_reference_media_tests`) |
| Watford 62-file extension | Watford | SSD or DSD | `real (runtime)` | `BGAME1_A.BBC` (genuine 41-entry Watford disc) + synthetic size-divergence regression and `watford_synthetic_tests` (DSD extensions, full catalogues, track-crossing reads and `.inf` metadata) |
| Solidisk / Opus catalogue | Solidisk/Opus | — | `real` | `data/solidisk_utils_side_a.ssd.gz` (real Solidisk DDFS utilities disc) + synthetic/Eagle_Empire external; DDFS is a density enhancement, not a layout change |
| HADFS (Harston) | HADFS | — | `real (runtime)` | mdfs.net `Software/HADFS/System.ssd` (unsupported variant; must not crash on DFS detection) |
| dir-char / locked attributes on real media | any | — | `real` | `data/apd01_ssd.ssd.gz` (G/U dir chars) |
| Catalogue bounds / broken entry | any | — | `synthetic` | `scenario_tests` |

`watford_synthetic_tests` preserves structural observations from a locally
inspected Watford DSD: independent 36/33-entry catalogues, an untitled second
side, extension files beyond side 0's declared size, and track-crossing data.
It also tests full 62-entry catalogues on both sides. Every image byte is
generated in memory, with invented filenames, metadata and payloads; the
tests contain no original file data or PII and need no external image.
Track placement and catalogue encoding are independent of the reader's
geometry helpers. These tests add synthetic evidence; the real-media status
still comes from the optional external fixture.

## External real-disc validation (opt-in)

`external_real_media_tests` pulls additional real discs at runtime from
**8bs.com**, **mdfs.net** and **bbcmicro.co.uk** (URL + SHA-256 pinned,
self-skips unless `ACORNFS_EXTERNAL_SOURCE` is set to a cache directory). It
validates the tool against more real media and feeds real-media facts back into
the matrix, but is **never** a committed-test dependency. Sources found:
- 8bs.com - public-domain BBC/Master discs (ADFS L/E and DFS).
- mdfs.net - J.G.Harston's freely-distributed software (Acorn DFS discs) and an
  HADFS System disc (unsupported variant).
- bbcmicro.co.uk - a large archive of real Acorn DFS game discs (single- and
  double-sided; a few are truncated/100 KB, which is an extra edge case).
- The BBC Lives mirror at rk.nvg.ntnu.no - genuine Watford DFS discs (the
  62-file extension), including one that exposed a real reader bug (see the
  divergence note under `dfs/watford-62`).
- Stardot - a Solidisk-recovered single-density game disc (sweh's archive
  posts).

Stardot.org.uk is primarily forums plus links out to mdfs.net and
retrosoftware.co.uk; it does not host a crawlable disc/hard-disc image archive,
so it is not used as a source. Genuine RISC OS hard-disc images (the source of
real new-map fragmentation) live on Arcology and are covered by the key-gated
fragmentation harness (see above).

## Authoring / independent generator

- `tools/GOLDEN_AUTHORING.md` - the recipe for creating real media with the
  official Acorn tools and handing goldens back for hashing.
- `tools/filecore_build.py` - an independent single-zone E-format builder used
  as a cross-check oracle and hard-disc/big-dir generator (see
  `tools/README.md`).

## Partitioned ADFS hard discs (recognised-by-analysis, not yet exercised)

Some real RISC OS hard discs are partitioned using **Acorn-specific schemes**
(not MBR/GPT): ICS/Baildon IDEFS, HCCS, SJ Research Nexus and Simtec. The
Arcology analysis code (`worker/arcworker/tools/partition.py`) documents their
layout, including that each partition carries its own FileCore boot block at
`partition_start + 0xC00` (disc record at `+0x1C0`), and the signatures:
- ICS - a "Part"-seeded checksum over sector 0 and `(start_sector,size_sector)`
  entries.
- HCCS - an `Andy` magic at boot-block `+0x1B0`, with contiguous partitions
  whose length is the disc record's `disc_size`.
- Nexus - a `Net1` magic at `0x20000`.
- Simtec - a signature detector.

We do not yet scan for these; a disc that is partitioned this way would not be
recognised at offset 0. The diagnostic drives probed (ConnerCP2024, ST3660A,
FireballSE1.2) use none of these signatures - they are either non-ADFS or
truncated/partial captures, which `info` now **detects and warns about**: an
image whose recorded disc size exceeds the actual image length is reported as
"image is truncated/partial" (see `report.rs`). Supporting these partition
schemes is a follow-up if such media is needed.

## Coverage tooling`cargo llvm-cov` (with `llvm-tools-preview`) reports line/branch coverage as a
secondary, report-only metric. This matrix — not a coverage threshold — is the
authoritative statement of *format* breadth.

Baseline against the full library test suite (unit + all always-on reference
images): **line 89.60%, branch 92.16%** (`cargo llvm-cov --summary-only --lib`).
The notably low modules are the CLI/binary and diagnostic paths that the
library test suite doesn't drive directly:

| Module | Line | Why |
|--------|------|-----|
| `cli.rs` | ~0% | CLI arg parsing exercised only by the binary, not the lib |
| `extract/log.rs` | ~10% | Extraction-log rendering used by binary output paths |
| `diagnostics.rs` | ~15% | Structured diagnostics serialisation |
| `verify.rs` | ~62% | Verification traversals partly covered via `verify_filecore_volume` |
| `extract/report.rs` | ~64% | Report/JSON rendering |

These aren't format cells; they'd need CLI/integration tests (or a public API)
to raise. The format matrix above is the meaningful coverage statement.
