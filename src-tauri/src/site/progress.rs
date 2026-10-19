//! Download progress lines from yt-dlp's `--progress-template`.
//!
//! Task: U1-10. See docs/tech-spec/url-import-design.md §5.3.
//!
//! The template is:
//!
//! ```text
//! download:PROG %(progress.downloaded_bytes)s %(progress.total_bytes)s %(progress.total_bytes_estimate)s %(progress.status)s
//! ```
//!
//! Measured (U0-9): a source with a known length fills `total_bytes` and leaves
//! the estimate as the literal `NA`; a source without one swaps them. Both cases
//! must produce a usable progress bar, and neither may panic on `NA`.

/// One parsed progress line.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub downloaded: u64,
    /// Denominator: `total_bytes`, else `total_bytes_estimate`.
    /// `None` means the size is genuinely unknown → indeterminate progress.
    pub total: Option<u64>,
    pub status: String,
}

impl Progress {
    /// Fraction complete in `0.0..=1.0`, or `None` when the total is unknown.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.total.filter(|total| *total > 0)?;
        Some((self.downloaded as f64 / total as f64).clamp(0.0, 1.0))
    }

    /// Whether this is the final line of a transfer.
    pub fn is_finished(&self) -> bool {
        self.status.eq_ignore_ascii_case("finished")
    }
}

/// `PROG` must be its own token; a title containing "PROG" must not match.
const PREFIX: &str = "PROG";

/// Parse one stdout line. Anything that is not a `PROG` line is `None`, which
/// the reader loop treats as "not for me" rather than as an error: yt-dlp prints
/// `[info]`/`[download]` chatter on the same stream.
pub fn parse_line(line: &str) -> Option<Progress> {
    let mut fields = line.split_whitespace();
    if fields.next()? != PREFIX {
        return None;
    }

    let downloaded = number(fields.next()?)?;
    let total_bytes = fields.next().and_then(number);
    let estimate = fields.next().and_then(number);
    // The status may itself contain no spaces (measured: `downloading`,
    // `finished`), but take the remainder so a future template stays readable.
    let status = fields.collect::<Vec<_>>().join(" ");

    Some(Progress {
        downloaded,
        // `total_bytes` wins; the estimate is the fallback (design §5.3).
        total: total_bytes.or(estimate).filter(|total| *total > 0),
        status,
    })
}

/// yt-dlp writes `NA` for an unknown numeric field; empty strings appear too.
fn number(field: &str) -> Option<u64> {
    field.trim().parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two real shapes measured in U0-9, verbatim.
    #[test]
    fn parses_a_line_with_a_known_total() {
        let progress = parse_line("PROG 1024 5408198 NA downloading").unwrap();
        assert_eq!(progress.downloaded, 1024);
        assert_eq!(progress.total, Some(5_408_198));
        assert_eq!(progress.status, "downloading");
        assert!(!progress.is_finished());
        let fraction = progress.fraction().unwrap();
        assert!((fraction - 1024.0 / 5_408_198.0).abs() < 1e-12);
    }

    #[test]
    fn parses_the_final_line() {
        let progress = parse_line("PROG 5408198 5408198 NA finished").unwrap();
        assert!(progress.is_finished());
        assert_eq!(progress.fraction(), Some(1.0));
    }

    #[test]
    fn falls_back_to_the_estimate_when_the_total_is_na() {
        let progress = parse_line("PROG 512 NA 2048 downloading").unwrap();
        assert_eq!(progress.total, Some(2048));
        assert_eq!(progress.fraction(), Some(0.25));
    }

    #[test]
    fn an_unknown_size_is_indeterminate_rather_than_an_error() {
        let progress = parse_line("PROG 512 NA NA downloading").unwrap();
        assert_eq!(progress.downloaded, 512);
        assert_eq!(progress.total, None);
        assert_eq!(
            progress.fraction(),
            None,
            "no denominator means the UI shows indeterminate progress"
        );

        // A zero total is the same thing, not a division by zero.
        let zero = parse_line("PROG 0 0 NA downloading").unwrap();
        assert_eq!(zero.fraction(), None);
    }

    #[test]
    fn ignores_every_non_prog_line() {
        for line in [
            "[download] Destination: /tmp/BV1.m4a",
            "[info] BV1: Downloading 1 format(s): 30280",
            "",
            "PROGRESS 1 2 3 downloading",
            " PROG",
        ] {
            // The leading-space case is not a PROG line because the first token
            // is `PROG` only after trimming — which we deliberately do NOT do:
            // real output never indents it, and trimming would also accept
            // `  PROG` inside a title line.
            assert_eq!(parse_line(line), None, "{line:?}");
        }
    }

    #[test]
    fn a_malformed_downloaded_field_is_not_a_progress_line() {
        // The first numeric field must parse; otherwise this is noise.
        assert_eq!(parse_line("PROG NA 100 NA downloading"), None);
        assert_eq!(parse_line("PROG abc 100 NA downloading"), None);
        // A missing status is still a usable line.
        let progress = parse_line("PROG 1 2 3").unwrap();
        assert_eq!(progress.status, "");
    }

    #[test]
    fn a_multi_word_status_is_kept_whole() {
        let progress = parse_line("PROG 1 2 NA downloading something").unwrap();
        assert_eq!(progress.status, "downloading something");
    }

    #[test]
    fn fraction_is_clamped() {
        let over = Progress {
            downloaded: 200,
            total: Some(100),
            status: "downloading".to_string(),
        };
        assert_eq!(over.fraction(), Some(1.0));
    }
}
