//! Synthetic regressions informed by a locally inspected Watford DSD.
//!
//! Retained observations: both sides have extension catalogues, their entry
//! counts differ (36/33), side 1 has no title, and side 0's base catalogue
//! declares a size ending before its extension files. Extension files cross
//! track boundaries and use locked attributes and a '+' directory character.
//! All titles, filenames, addresses, lengths and payloads below are invented.
//! No source image, original file metadata, payload or PII is used at runtime
//! or included here.
//!
//! Catalogue bytes and track placement are authored here without testutil or
//! the reader's geometry/encoding helpers, to avoid a shared round-trip bug.

use std::io::Cursor;

use tempfile::tempdir;

use crate::diagnostics::Diagnostics;
use crate::extract::log::{ExtractionLog, LogEntry};
use crate::extract::report::build_dfs_report;
use crate::extract::walker::{BrokenDirPolicy, ExtractOptions, walk_and_extract};
use crate::format::dfs::DfsFs;
use crate::io::rescue::BadSectorPolicy;
use crate::verify::verify;

const SECTOR_BYTES: usize = 256;
const SIDE_SECTORS: usize = 800;
const OBSERVED_COUNTS: [usize; 2] = [36, 33];
const SHORT_TOTAL: usize = 314;

fn payload(side: usize, index: usize) -> Vec<u8> {
    (0..1801 + index * 5 + side * 23)
        .map(|offset| (offset * 3 + offset / 256 * 17 + side * 67 + index * 31) as u8)
        .collect()
}

fn names(index: usize) -> (String, String) {
    if index >= 31 && index % 2 == 1 {
        (format!("+.F{index:02}"), format!("&.F{index:02}"))
    } else {
        (format!("F{index:02}"), format!("F{index:02}"))
    }
}

fn addresses(side: usize, index: usize) -> (u32, u32) {
    let offset = (side * 256 + index) as u32;
    (0xFF1200 + offset, 0xFF3400 + offset)
}

fn synthetic_dsd(counts: [usize; 2]) -> Vec<u8> {
    let mut image = vec![0; SIDE_SECTORS * 2 * SECTOR_BYTES];
    for (side, count) in counts.into_iter().enumerate() {
        assert!((32..=62).contains(&count));
        let mut logical = vec![0; SIDE_SECTORS * SECTOR_BYTES];
        if side == 0 {
            logical[..8].copy_from_slice(b"SYNTHETC");
        }
        for block in 0..2 {
            let base = block * 512;
            if block == 1 {
                logical[base..base + 8].fill(0xAA);
            }
            let total = if side == 0 && block == 0 {
                SHORT_TOTAL
            } else {
                SIDE_SECTORS
            };
            logical[base + 260] = 0x12;
            logical[base + 261] = if block == 0 { 31 } else { (count - 31) as u8 } * 8;
            logical[base + 262] = ((total >> 8) as u8) | if side == 0 { 0x30 } else { 0 };
            logical[base + 263] = total as u8;
        }

        for index in 0..count {
            let block = if index < 31 { 0 } else { 512 };
            let slot = index % 31;
            let name_offset = block + 8 + slot * 8;
            let info_offset = name_offset + 256;
            let name = format!("F{index:02}");
            logical[name_offset..name_offset + 7].fill(b' ');
            logical[name_offset..name_offset + name.len()].copy_from_slice(name.as_bytes());
            let directory = if index >= 31 && index % 2 == 1 {
                b'+'
            } else {
                b'$'
            };
            logical[name_offset + 7] = directory | if index % 2 == 1 { 0x80 } else { 0 };

            let data = payload(side, index);
            // Each run begins at track sector 4 and crosses into the next
            // track. Ten-sector spacing leaves room without overlapping files.
            let start = 4 + index * 10;
            let (load, exec) = addresses(side, index);
            logical[info_offset..info_offset + 2].copy_from_slice(&(load as u16).to_le_bytes());
            logical[info_offset + 2..info_offset + 4].copy_from_slice(&(exec as u16).to_le_bytes());
            logical[info_offset + 4..info_offset + 6]
                .copy_from_slice(&(data.len() as u16).to_le_bytes());
            // Both addresses have the all-ones high field; lengths fit 16 bits.
            logical[info_offset + 6] = 0xCC | (start >> 8) as u8;
            logical[info_offset + 7] = start as u8;
            logical[start * SECTOR_BYTES..start * SECTOR_BYTES + data.len()].copy_from_slice(&data);
        }

        // Assemble complete alternating tracks, independently of DfsGeometry.
        for (track, data) in logical.as_chunks::<2560>().0.iter().enumerate() {
            let physical = track * 5120 + side * 2560;
            image[physical..physical + 2560].copy_from_slice(data);
        }
    }
    image
}

#[test]
fn watford_dsd_has_independent_extension_catalogues() {
    // Also exercise the final slot of each extension block at full capacity.
    for counts in [OBSERVED_COUNTS, [62, 62]] {
        let fs = DfsFs::open(Cursor::new(synthetic_dsd(counts))).unwrap();
        assert!(fs.double_sided);
        assert_eq!(fs.catalogues.len(), 2);
        for (side, cat) in fs.catalogues.iter().enumerate() {
            assert!(cat.watford, "side {side}");
            assert_eq!(cat.entries.len(), counts[side]);
            assert_eq!(cat.title, if side == 0 { "SYNTHETC" } else { "" });
            assert_eq!(cat.boot_option, if side == 0 { 3 } else { 0 });
            assert_eq!(
                cat.total_sectors,
                if side == 0 { SHORT_TOTAL as u32 } else { 800 }
            );
            for (index, entry) in cat.entries.iter().enumerate() {
                assert_eq!(entry.name, format!("F{index:02}"));
                assert_eq!(entry.start_sector, (4 + index * 10) as u32);
                assert_eq!(entry.in_bounds(cat.total_sectors), side == 1 || index < 31);
            }
        }
    }
}

#[test]
fn watford_dsd_short_declared_side_verifies_with_report_warning() {
    let mut fs = DfsFs::open(Cursor::new(synthetic_dsd(OBSERVED_COUNTS))).unwrap();
    let report = build_dfs_report(&mut fs).unwrap();
    assert_eq!(report.dir_type, "double-sided");
    assert_eq!(
        report.disc_size,
        Some(((SHORT_TOTAL + SIDE_SECTORS) * SECTOR_BYTES) as u64)
    );
    assert_eq!(report.directory_warnings.len(), 1);
    assert!(report.directory_warnings[0].contains("diverges from the image size"));
    assert!(report.root_check_byte_ok);
    assert!(report.broken_directories.is_empty());

    let mut diagnostics = Diagnostics::default();
    let verified = verify(&mut fs, &mut diagnostics).unwrap();
    assert_eq!(
        (verified.directories, verified.files, verified.unreadable),
        (3, 69, 0)
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn watford_dsd_extracts_track_crossing_files_and_extension_metadata() {
    for counts in [OBSERVED_COUNTS, [62, 62]] {
        let mut fs = DfsFs::open(Cursor::new(synthetic_dsd(counts))).unwrap();
        let dir = tempdir().unwrap();
        let opts = ExtractOptions {
            output_dir: dir.path().to_owned(),
            write_inf: true,
            dry_run: false,
            broken_dir_policy: BrokenDirPolicy::Fail,
            bad_sector_policy: BadSectorPolicy::NullFill,
            rescue_map: None,
        };
        let mut log = ExtractionLog::default();
        let summary = walk_and_extract(&mut fs, &opts, &mut log).unwrap();
        let file_count = counts.iter().sum::<usize>();
        assert_eq!(summary.files_extracted, file_count as u64);
        assert_eq!(summary.files_skipped, 0);
        assert_eq!(summary.dirs_created, 3);
        assert_eq!(log.entries.len(), file_count);
        assert!(
            log.entries
                .iter()
                .all(|e| matches!(e, LogEntry::Extracted { .. }))
        );

        let mut total_bytes = 0;
        for (side, count) in counts.into_iter().enumerate() {
            let side_dir = dir.path().join(format!("Side{side}"));
            assert_eq!(std::fs::read_dir(&side_dir).unwrap().count(), count * 2);
            for index in 0..count {
                let data = payload(side, index);
                let (original_name, host_name) = names(index);
                let path = side_dir.join(&host_name);
                assert_eq!(std::fs::read(&path).unwrap(), data, "{}", path.display());
                total_bytes += data.len() as u64;
                let (load, exec) = addresses(side, index);
                let attrs = if index % 2 == 1 { 4 } else { 0 };
                let expected_inf = format!(
                    "{original_name} {load:08x} {exec:08x} {:08x} {attrs:02x} CRC32={:08X}",
                    data.len(),
                    crc32fast::hash(&data),
                );
                let inf =
                    std::fs::read_to_string(side_dir.join(format!("{host_name}.inf"))).unwrap();
                assert_eq!(inf.trim_end(), expected_inf, "{}", path.display());
            }
        }
        assert_eq!(summary.total_bytes, total_bytes);
    }
}
