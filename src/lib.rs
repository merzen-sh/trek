//! Archive tooling for `trek`.
//!
//! The `pack` subcommand builds deterministic ZIP archives from `.pack`
//! include-pattern files and can preview or report on the validated file list.

pub mod archive;
pub mod cli;
pub mod manifest;
pub mod pack;
pub mod report;

#[cfg(test)]
mod test_support;
