//! Archive tooling for `trek`.
//!
//! The `pack` subcommand builds deterministic ZIP archives from `.pack`
//! include-pattern files and can preview or report on the validated file list.
//! The `version` subcommand reads or bumps the version in `fxmanifest.lua`, and
//! `release` combines a version bump with archive creation and a Markdown report.

pub mod archive;
pub mod cli;
pub mod manifest;
pub mod pack;
pub mod release;
pub mod report;
pub mod version;

#[cfg(test)]
mod test_support;
