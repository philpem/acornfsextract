//! Differential harness against an independent second reader.
//!
//! We (acornfsextract) are the *reference* reader; this test runs our reader
//! and ejmount/acorn-dfs over the same images and reports where they disagree.
//! Because acorn-dfs is a fully independent implementation (different author,
//! different parse paths), a disagreement is genuine cross-validation - far
//! stronger than the in-repo synthetic builder round-trip, which shares this
//! project's author and could never expose a shared misinterpretation.
//!
//! The harness treats a disagreement as a *differential adjudication*, not an
//! assertion: for an awkward/uncovered cell it may be *us* that is wrong
//! (the guide's real media once corrected exactly such an assumption about
//! old-map hard-disc directory layout), so on mismatch it dumps both readers'
//! raw readings rather than failing acorn-dfs.
//!
//! Opt-in: run with `cargo test --features differential` so a default
//! `cargo test` never fetches the git dependency (or needs network). This
//! mirrors the report-only posture of the format-coverage matrix.
#![cfg(feature = "differential")]

use std::io::Cursor;
use std::path::Path;

use acorn_dfs::new_map::sys_structures::FormatE;
use acorn_dfs::new_map::FaultValue;
use acornfsextract::extract::report::{build_dfs_report, build_report};
use acornfsextract::format::dfs::DfsFs;
use acornfsextract::format::filecore::FileCoreFs;
use acornfsextract::format::fs::FileSystem;

const DATA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/");

/// One recovered file: RISC OS path, declared length, content CRC-32, and
/// (for our reader) the resolved disc extents - the bit of reading that is
/// most likely to diverge and therefore the most useful to adjudicate.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileReading {
    name: String,
    len: u64,
    crc: u32,
    extents: Vec<(u64, u64)>,
}

fn hash(bytes: &[u8]) -> u32 {
    crc32fast::hash(bytes)
}

/// Our reading of a disc: metadata report plus the full file inventory.
struct OurReading {
    identity: String,
    report_json: String,
    files: Vec<FileReading>,
}

/// The independent reader's reading: parse status plus (if parsed) its file
/// inventory and the serialised map it recovered.
struct TheirReading {
    parsed: bool,
    note: String,
    map_json: Option<String>,
    files: Vec<FileReading>,
}

fn own_filecore_reading(bytes: &[u8]) -> Option<OurReading> {
    let mut fs = FileCoreFs::open(Cursor::new(bytes.to_vec())).ok()?;
    let report = build_report(&mut fs).ok()?;
    let identity = format!(
        "FileCore map={} dir={} name={:?} size={:?} sector={:?} zone_ck={:?} cross={:?}",
        report.map_type,
        report.dir_type,
        report.disc_name,
        report.disc_size,
        report.sector_size,
        report.zone_checksum_ok_count,
        report.cross_check_ok,
    );
    let report_json = serde_json::to_string_pretty(&report).unwrap();
    let root = fs.root().expect("root");
    let mut files = Vec::new();
    let mut stack = vec![(root, "$".to_string())];
    while let Some((dir_obj, path)) = stack.pop() {
        let Ok(listing) = fs.list(&dir_obj) else { continue };
        for obj in listing.objects {
            let child = format!("{path}.{}", obj.name);
            if obj.is_directory {
                stack.push((obj, child));
            } else {
                let mut content = Vec::new();
                if fs
                    .read_object(&obj, &mut |_, b| {
                        content.extend_from_slice(b);
                        Ok(())
                    })
                    .is_ok()
                {
                    files.push(FileReading {
                        name: child,
                        len: content.len() as u64,
                        crc: hash(&content),
                        extents: obj.extents.iter().map(|e| (e.disc_addr, e.len)).collect(),
                    });
                }
            }
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Some(OurReading {
        identity,
        report_json,
        files,
    })
}

fn own_dfs_reading(bytes: &[u8]) -> Option<OurReading> {
    let mut fs = DfsFs::open(Cursor::new(bytes.to_vec())).ok()?;
    let report = build_dfs_report(&mut fs).ok()?;
    let identity = format!(
        "DFS sided={} name={:?} size={:?}",
        report.dir_type, report.disc_name, report.disc_size
    );
    Some(OurReading {
        identity,
        report_json: serde_json::to_string_pretty(&report).unwrap(),
        files: Vec::new(),
    })
}

fn own_reading(bytes: &[u8]) -> Option<OurReading> {
    own_filecore_reading(bytes).or_else(|| own_dfs_reading(bytes))
}

/// Try the independent reader. It currently only implements new-map Format E,
/// so any other image is reported as not-handled rather than a failure. It is
/// wrapped in `catch_unwind` because a strict reader may `debug_assert!` (in a
/// debug build) on an image that does not meet its assumptions (e.g. a
/// fragment not spanning whole sectors); a panic must never take the whole
/// harness down, it is reported as adjudication like any other rejection.
fn their_reading(bytes: &[u8]) -> TheirReading {
    let disk = match std::panic::catch_unwind(|| FormatE::parse(bytes)) {
        Ok(Ok(d)) => d,
        Ok(Err(e)) => {
            return TheirReading {
                parsed: false,
                note: format!("rejected by the new-map reader ({})", root_cause(&e)),
                map_json: None,
                files: Vec::new(),
            };
        }
        Err(panic) => {
            let msg = panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic".into());
            return TheirReading {
                parsed: false,
                note: format!("panicked in the new-map reader ({msg})"),
                map_json: None,
                files: Vec::new(),
            };
        }
    };
    let map_json = Some(disk.get_map_json());
    let mut files = Vec::new();
    for path in disk.entries(None) {
        let mut content = Vec::new();
        let Ok(FaultValue(res, _)) = disk.get_file(&path, &mut content) else {
            continue;
        };
        if res.is_ok() {
            files.push(FileReading {
                name: path.to_string(),
                len: content.len() as u64,
                crc: hash(&content),
                extents: Vec::new(),
            });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    TheirReading {
        parsed: true,
        note: "parsed".to_string(),
        map_json,
        files,
    }
}

/// Compares recovered content (path + declared length + content CRC), NOT the
/// resolved disc extents - the independent reader deliberately does not expose
/// extents, so extents are only shown in the adjudication dump, never used to
/// decide a match.
fn files_match(a: &[FileReading], b: &[FileReading]) -> bool {
    let key = |f: &FileReading| (f.name.clone(), f.len, f.crc);
    let ka: Vec<_> = a.iter().map(key).collect();
    let kb: Vec<_> = b.iter().map(key).collect();
    ka == kb
}

/// Extracts the innermost/most-specific `cause:` value from a winnow parse
/// error, dropping the huge `input: "\0..."` blobs (for a disc image, the
/// whole file). E.g. yields `UnacceptableSectorSize(12)`.
fn root_cause(e: &dyn std::fmt::Debug) -> String {
    let full = format!("{e:?}");
    let needle = "cause: Some(";
    let Some(start) = full.rfind(needle) else {
        return full.chars().take(80).collect();
    };
    let inner = &full[start + needle.len()..];
    let mut depth = 0usize;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return inner[..i].to_string();
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    inner.to_string()
}

/// Adjudication dump: both readers' raw readings for one image, so a
/// disagreement can be decided rather than assumed. Printed (not asserted).
fn adjudicate(label: &str, ours: Option<&OurReading>, theirs: &TheirReading) {
    eprintln!("--- {label}");
    match ours {
        None => eprintln!("    our reader : did not recognise as FileCore/DFS"),
        Some(o) => {
            eprintln!("    our reader : {}", o.identity);
            eprintln!("      our metadata report (first 40 lines):");
            for line in o.report_json.lines().take(40) {
                eprintln!("        {line}");
            }
            if !o.files.is_empty() {
                eprintln!("      our files ({}):", o.files.len());
                for f in &o.files {
                    eprintln!("        {} len={} crc={:08x} extents={:?}", f.name, f.len, f.crc, f.extents);
                }
            }
        }
    }
    if theirs.parsed {
        eprintln!("    other      : parsed");
        eprintln!("      other files ({}):", theirs.files.len());
        for f in &theirs.files {
            eprintln!("        {} len={} crc={:08x}", f.name, f.len, f.crc);
        }
        if let Some(map) = &theirs.map_json {
            eprintln!("      other recovered map (first 40 lines):");
            for line in map.lines().take(40) {
                eprintln!("        {line}");
            }
        }
    } else {
        eprintln!("    other      : {}", theirs.note);
    }
    eprintln!();
}

fn compare(label: &str, ours: Option<&OurReading>, theirs: &TheirReading) {
    let our_ok = ours.is_some();
    let their_ok = theirs.parsed;
    if !our_ok || !their_ok {
        // A side that didn't parse can't be adjudicated as a content
        // disagreement; just report status and stop there.
        adjudicate(label, ours, theirs);
        return;
    }
    let o = ours.unwrap();
    if files_match(&o.files, &theirs.files) {
        eprintln!("=== {label}");
        eprintln!("    both parsed; recovered file inventory MATCHES ({} files)", o.files.len());
        eprintln!();
    } else {
        // Adjudication needed - dump both readings.
        eprintln!("=== {label}  <-- DIFFERS");
        adjudicate(label, Some(o), theirs);
    }
}

fn load_corpus(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut names: Vec<_> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |x| x == "gz"))
        .map(|e| e.path())
        .collect();
    names.sort();
    for p in names {
        let file = std::fs::File::open(&p).unwrap();
        let mut gz = flate2::read::GzDecoder::new(file);
        let mut bytes = Vec::new();
        if std::io::Read::read_to_end(&mut gz, &mut bytes).is_ok() {
            out.push((p.file_name().unwrap().to_string_lossy().into_owned(), bytes));
        }
    }
    out
}

#[test]
fn differential_corpus_matrix() {
    eprintln!("== Differential harness (corpus) ==");
    let corpus = load_corpus(Path::new(DATA_DIR));
    if corpus.is_empty() {
        eprintln!("  (no corpus fixtures found in {DATA_DIR})");
    }
    for (label, bytes) in &corpus {
        let ours = own_reading(bytes);
        let theirs = their_reading(bytes);
        compare(label, ours.as_ref(), &theirs);
    }
}

#[test]
fn differential_generator_matrix() {
    use acornfsextract::testutil::*;

    eprintln!("== Differential harness (synthetic generator matrix) ==");
    // The other reader (acorn-dfs) only accepts new-map discs with
    // 256/512/1024/2048-byte sectors (log2 8..11) and asserts that every
    // fragment is a whole sector, so the new-map synthetics are generated with
    // 1024-byte sectors and the allocation unit equal to the sector size
    // (log2_bpmb == log2_sec_size == 10). This lets both readers agree on the
    // generated image, which is the point of the harness: proving the
    // generator's output is independently valid, not self-confirming.
    let cfg = NewMapConfig { log2_sector_size: 10, log2_bpmb: 10, ..NewMapConfig::default() };
    let mut images: Vec<(String, Vec<u8>)> = Vec::new();

    let img = build_new_map_disc(
        vec![
            SynthEntry::File(SynthFile::plain("plain.txt", b"hello newmap world")),
            SynthEntry::dir(
                "subdir",
                vec![SynthEntry::File(SynthFile::plain("nested.bin", b"abc"))],
            ),
        ],
        &cfg,
    );
    images.push(("synthetic:newmap-small-dir-E".into(), img.bytes));

    let cfg_big = NewMapConfig {
        log2_sector_size: 10,
        log2_bpmb: 10,
        big_dirs: true,
        ..NewMapConfig::default()
    };
    let img = build_new_map_disc(
        vec![SynthEntry::File(SynthFile::plain("big.txt", b"in a big directory"))],
        &cfg_big,
    );
    images.push(("synthetic:newmap-big-dir".into(), img.bytes));

    let (img, _expected) = build_random_fragmented_disc(0x5EED, &cfg, 8);
    images.push(("synthetic:newmap-fragmented".into(), img.bytes));

    let img = build_old_map_disc(
        vec![SynthFile::plain("old.txt", b"old map data")],
        false,
        "OLDMAP",
    );
    images.push(("synthetic:oldmap-new-dir-D".into(), img.bytes));

    let img = build_sml_disc(
        &[SynthFile::plain("sml.txt", b"sml data spanning maybe a track")],
        640,
        "SML",
    );
    images.push(("synthetic:sml-L".into(), img.bytes));

    let img = build_dfs_disc(vec![DfsSideSpec::new(
        "TEST",
        vec![DfsFile::plain("FILE", b"dfs data")],
    )]);
    images.push(("synthetic:dfs-ssd".into(), img.bytes));

    for (label, bytes) in &images {
        let ours = own_reading(bytes);
        let theirs = their_reading(bytes);
        compare(label, ours.as_ref(), &theirs);
    }
}
