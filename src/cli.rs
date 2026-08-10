//! Clap arguments shared by the command-line front ends.

use crate::Verbosity;
use clap::Args;

/// The repeatable `-v` flag, for `#[command(flatten)]` into an application's
/// own argument struct.
///
/// Flattening rather than exposing a bare `u8` keeps the help text identical
/// across binaries, which is the thing that actually drifted when each one
/// declared its own copy.
// clap derives the host command's `about` from a flattened struct's doc
// comment, so without this the rustdoc above lands at the top of every
// consuming binary's `--help` output.
#[derive(Debug, Copy, Clone, Args)]
#[command(about = None, long_about = None)]
pub struct VerbosityArgs {
    /// Increase verbosity (-v for debug, -vv for trace).
    #[arg(short = 'v', action = clap::ArgAction::Count)]
    pub verbosity: u8,
}

impl From<VerbosityArgs> for Verbosity {
    fn from(value: VerbosityArgs) -> Self {
        value.verbosity.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Verbosity, cli::VerbosityArgs};
    use clap::{CommandFactory, Parser};

    #[derive(Debug, Parser)]
    struct TestArgs {
        #[command(flatten)]
        verbosity: VerbosityArgs,
    }

    #[test]
    fn test_flattening_leaves_the_host_commands_description_alone() {
        let help = TestArgs::command().render_long_help().to_string();

        // clap takes a flattened struct's doc comment as the host command's
        // `about` unless told not to, which put this crate's rustdoc at the top
        // of every consuming binary's --help.
        assert!(!help.contains("Flattening rather than"), "{help}");
        assert!(help.starts_with("Usage:"), "{help}");
    }

    #[test]
    fn test_the_flag_still_documents_itself() {
        let help = TestArgs::command().render_long_help().to_string();

        assert!(help.contains("Increase verbosity"), "{help}");
    }

    fn verbosity_from(args: &[&str]) -> Verbosity {
        TestArgs::parse_from(args).verbosity.into()
    }

    #[test]
    fn test_no_flag_is_info() {
        assert!(matches!(verbosity_from(&["test"]), Verbosity::Info));
    }

    #[test]
    fn test_single_flag_is_debug() {
        assert!(matches!(verbosity_from(&["test", "-v"]), Verbosity::Debug));
    }

    #[test]
    fn test_double_flag_is_trace() {
        assert!(matches!(verbosity_from(&["test", "-vv"]), Verbosity::Trace));
    }

    #[test]
    fn test_beyond_double_flag_stays_trace() {
        assert!(matches!(
            verbosity_from(&["test", "-vvvv"]),
            Verbosity::Trace
        ));
    }
}
