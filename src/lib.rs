pub mod cli;
pub mod diagnostics;
pub mod error;
pub mod extract;
pub mod format;
pub mod io;
pub mod model;
pub mod sidecar;
pub mod util;
pub mod verify;
pub mod xlate;

#[cfg(test)]
pub mod testutil;

#[cfg(test)]
mod arcology_corpus_tests;

#[cfg(test)]
mod real_media_tests;

#[cfg(test)]
mod reference_media_tests;

#[cfg(test)]
mod scenario_tests;

#[cfg(test)]
mod format_coverage_tests;

#[cfg(test)]
mod dfs_reference_media_tests;

#[cfg(test)]
mod solidisk_reference_media_tests;

#[cfg(test)]
mod watford_synthetic_tests;

#[cfg(test)]
mod oldmap_hard_disc_tests;

#[cfg(test)]
mod corpus_tests;

#[cfg(test)]
mod external_real_media_tests;

#[cfg(test)]
mod afs_real_media_tests;

#[cfg(test)]
mod real_media_common;
