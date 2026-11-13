// The newest sample of a logger, for the Watermark's ping (the F3 chart keeps
// its samples private to this module).
impl super::SampleLogger {
    /// The first value of the most recent sample, if any.
    pub fn clickgui_latest(&self) -> Option<i64> {
        self.samples.back().map(|sample| sample[0])
    }
}
