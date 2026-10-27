//! Helpers shared by the unit tests

use std::io::Write;

/// A writer that accepts `left` more bytes and then fails
pub(crate) struct FailsAfter {
    pub(crate) left: usize,
    pub(crate) written: Vec<u8>,
}

impl Write for FailsAfter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.left == 0 {
            return Err(std::io::Error::other("disk full"));
        }
        let n = buf.len().min(self.left);
        self.written.extend_from_slice(&buf[..n]);
        self.left -= n;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Run `print` against a writer that fails after every possible number of
/// bytes: it must write exactly what fits, in order, and hand back the
/// writer's error instead of carrying on or panicking.
pub(crate) fn assert_stops_at_a_failed_write(
    print: impl Fn(&mut FailsAfter) -> std::io::Result<()>,
) {
    let mut all = FailsAfter {
        left: usize::MAX,
        written: Vec::new(),
    };
    print(&mut all).unwrap();
    all.flush().unwrap();
    let full = all.written;
    assert!(!full.is_empty());
    for limit in 0..full.len() {
        let mut out = FailsAfter {
            left: limit,
            written: Vec::new(),
        };
        let err = print(&mut out).expect_err(&format!("limit {limit}"));
        assert_eq!(err.to_string(), "disk full", "limit {limit}");
        assert_eq!(out.written, full[..limit], "limit {limit}");
    }
}
