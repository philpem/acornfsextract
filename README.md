# acornfsextract

A read-only extractor and inspector for Acorn FileCore (ADFS) and DFS disc
images, written in Rust. It understands the full FileCore format matrix -
old-map S/M/L floppies, D, E/F floppies, big-directory (E+/F+/G) discs and
raw hard-disc images - as well as DFS single- and double-sided images.

The FileCore format is documented externally and is **not** vendored here.
The authoritative specification is the FileCore Technical Guide, section
references in the code (e.g. "guide §2.4") point at it:

- https://github.com/philpem/arcology/blob/master/doc/format_info/acorn32bit/filecore_guide.md

## Usage

```sh
# Disc metadata (format, disc name, size, structural health)
acornfsextract info <image> [--format text|json]

# Full structural verification with typed diagnostics
acornfsextract verify <image> [--format text|json] [--diagnostics <path>]

# Extract every file into a directory tree
acornfsextract extract <image> --output <dir> [options]
```

`extract` options:

| Flag | Meaning |
|------|---------|
| `--output <dir>` | Destination directory (required) |
| `--rescue-map <path>` | ddrescue/gddrescue mapfile marking known bad sectors |
| `--bad-sectors skip\|null\|marker` | How to handle bad sectors (default `null`) |
| `--on-broken-directory fail\|skip\|recover` | Policy for a broken directory (default `recover`) |
| `--inf` | Write a `.inf` sidecar per extracted file |
| `--log <path>` | Write an extraction log (bad sectors, skips, anomalies) |
| `--dry-run` | List what would be extracted without writing |
| `--partition <label>` | Extract only the named partition (see below) |
| `--format text\|json` | Output format |

## Partitions

An image may contain more than one filesystem volume. A **hybrid** disc is an
ADFS filesystem that also carries an embedded Acorn File Server (AFS0)
partition; such an image exposes two partitions, `ADFS` and `AFS`. The
`info` and `verify` commands report every partition found. `extract`, when
more than one partition is present, extracts each into a subdirectory named
after that partition (e.g. `<dir>/ADFS/...` and `<dir>/AFS/...`). Use
`--partition ADFS` (or `--partition AFS`) to extract a single partition
directly into the output directory (the file layout then matches the
single-volume case).

FileCore and AFS are both signature/structurally-detected and always
attempted; DFS - which has no magic number, only structural plausibility -
is only tried as a final fallback.

Note that the `--format json` output for `info`/`verify`/`extract` is always
a **list** (one element per partition), even for a single-volume image, so
tooling should not expect a bare object.



## Tests

```sh
cargo test
```

The suite includes always-on regression tests against real disc images
committed to `data/` - the ADFS S/M/L, D, E and F format fixtures, plus two
genuine Acorn DFS discs (`apd01_ssd` and `8bs0_dsd`). The ADFS fixtures all
contain the same files, which lets the tests assert byte-identical extraction
across formats and thereby validate the S/M/L sector-interleave translation.
The DFS fixtures are the only non-synthetic ground truth for the DFS backend
and cover single- and double-sided media plus a real `G`/`U`
directory-character mix. See `data/README.md` for provenance.

A **format-coverage matrix** (report-only) is printed by
`src/format_coverage_tests.rs`; it documents which on-disk format cells are
covered by real media versus synthetic-only versus uncovered, and is the
authoritative statement of format breadth (see `docs/FORMAT_COVERAGE.md`).

An optional stress harness pulls copyrighted corpus images at runtime only
when an Arcology API key is set (it self-skips otherwise, so CI never needs
network or a key):

```sh
ACORNFS_ARCOLOGY_KEY=<key> cargo test stress_against_arcology_corpus
# Real new-map fragmentation coverage (many sampled .dd hard discs contain
# fragmented files; nothing is pinned or committed):
ACORNFS_ARCOLOGY_KEY=<key> cargo test hard_disc_fragmentation_against_arcology_corpus
```

Configurable via `ACORNFS_ARCOLOGY_API` (base URL), `ACORNFS_ARCOLOGY_MAX`
(sample size), `ACORNFS_ARCOLOGY_EXT` (extensions to sample) and
`ACORNFS_ARCOLOGY_MAXSIZE` (skip larger artefacts).

Separately, opt-in targets validate additional pinned real discs (Acorn File
Server Level 2/3 + hybrids, plus ADFS/DFS media from 8bs.com, mdfs.net,
bbcmicro.co.uk and Stardot). Locally they fetch each disc from its source and
cache it; the files are never bundled:

```sh
# Local: fetches (or reads from the cache) and validates, with the SHA-256
# checked against the pinned manifest.
ACORNFS_EXTERNAL_SOURCE=/tmp/cache cargo test --lib validates_real_afs_images
ACORNFS_EXTERNAL_SOURCE=/tmp/cache cargo test --lib validates_additional_real_discs
```

`ACORNFS_EXTERNAL_SOURCE` is a directory used as the cache.

## Real-media bundle for CI

CI should not poll each forum/mirror (Stardot requires a login and is
rate-limited). Instead the workflow downloads a **single bundle ZIP** that the
maintainer hosts, and validates every contained image against its pinned
SHA-256. Nothing is fetched per source in CI.

To (re)generate the bundle locally:

```sh
ACORNFS_EXTERNAL_SOURCE=/tmp/cache cargo test --lib validates_real_afs_images
ACORNFS_EXTERNAL_SOURCE=/tmp/cache cargo test --lib validates_additional_real_discs
cd /tmp/cache && zip real-media.zip ./*    # then upload real-media.zip
```

Set the repo secret **`REAL_MEDIA_URL`** to the bundle's download URL. While
that secret is unset the real-media job is a no-op (it compiles and the
harness is skipped). `ACORNFS_CI=1` (set by the workflow) disables the
per-source fetch; if the bundle is absent the entry is skipped silently.

### Locking the bundle to GitHub Actions runners

The bundle URL is only fetched from CI, but the endpoint is public. To prevent
casual downloads you can either:

- Include a token/query parameter in `REAL_MEDIA_URL` (e.g. a signed or
  `?token=...` URL), or
- IP-allowlist GitHub Actions runner ranges on your server using the CIDRs
  published at https://api.github.com/meta (the `actions` key), which is the
  practical way to restrict to runners only.

A broader opt-in sweep against any local corpus of real disc images (e.g. an
archive under `/mnt/nfs`) opens and verifies every recognisable Acorn disc and
reports which format cells are actually exercised - no files are committed and
no hash is pinned, so it is purely a real-media coverage/health check:

```sh
ACORNFS_CORPUS_DIR=/mnt/nfs cargo test --lib scan_corpus_for_format_coverage
```

An opt-in **differential harness** (report-only, never a CI gate) runs this
reader and an independent second reader, [ejmount/acorn-dfs](https://github.com/ejmount/acorn-dfs),
over the same committed corpus and synthetic matrix and reports where their
recovered file inventories diverge. Because the other reader is *independent*
(a different author and parse path), a divergence is genuine cross-validation
rather than the self-confirming builder round-trip; on a divergence it dumps
both readers' raw readings so a cell can be adjudicated rather than assumed.
It also wraps the other reader in a panic guard, so a `debug_assert!`-style
rejection is reported as adjudication instead of taking the harness down.

The synthetic new-map generator was corrected in the same work so that its
allocation-map bit-length exactly matches the disc's allocation-unit count
(and fragments occupy whole sectors): the previous sizing left the free-space
map inconsistent, which a strict validating reader notices even though this
one never did. The generator matrix is now produced with 1024-byte sectors and
the allocation unit equal to the sector size, which is the geometry the other
reader accepts, so its generated `E` disc — not just the committed real-media
fixture — cross-validates against the independent reader.

It requires fetching the git dependency, so it is feature-gated:

```sh
cargo test --features differential --test differential -- --nocapture
```

An optional coverage tool (report-only, not a CI gate) is available:

```sh
cargo llvm-cov --summary-only --lib   # requires cargo-llvm-cov + llvm-tools-preview
```

Golden images (real media created with official Acorn tools) can be produced
and validated by following `tools/GOLDEN_AUTHORING.md`; the donor images those
steps need are emitted by `ACORNFS_DONOR_OUT=/path cargo test --lib emit_golden_donors_if_requested`.
