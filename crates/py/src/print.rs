use crate::ctx::Ctx;
use monty_types::{ExcType, MontyException, PrintWriterCallback};
use std::borrow::Cow;

/// Collects `print()` output (up to `max` bytes, the rest is counted) and stops
/// a loop that never calls the host once the run is cancelled or out of time.
pub struct Collector<'a> {
    pub buf: String,
    pub dropped: usize,
    max: usize,
    ctx: &'a Ctx<'a>,
}

impl<'a> Collector<'a> {
    pub fn new(max: usize, ctx: &'a Ctx<'a>) -> Self {
        Self {
            buf: String::new(),
            dropped: 0,
            max,
            ctx,
        }
    }

    fn put(&mut self, s: &str) {
        let room = self.max.saturating_sub(self.buf.len());
        if s.len() <= room {
            self.buf.push_str(s);
            return;
        }
        let mut cut = room;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        self.buf.push_str(&s[..cut]);
        self.dropped += s.len() - cut;
    }
}

impl PrintWriterCallback for Collector<'_> {
    fn stdout_write(&mut self, output: Cow<'_, str>) -> Result<(), MontyException> {
        self.put(&output);
        self.poll_flush()
    }

    fn stdout_push(&mut self, end: char) -> Result<(), MontyException> {
        let mut b = [0; 4];
        self.put(end.encode_utf8(&mut b));
        Ok(())
    }

    fn poll_flush(&mut self) -> Result<(), MontyException> {
        if self.ctx.cancelled() {
            Err(MontyException::new(
                ExcType::KeyboardInterrupt,
                Some("cancelled".into()),
            ))
        } else if self.ctx.expired() {
            Err(MontyException::new(
                ExcType::TimeoutError,
                Some("the call timed out".into()),
            ))
        } else {
            Ok(())
        }
    }
}
