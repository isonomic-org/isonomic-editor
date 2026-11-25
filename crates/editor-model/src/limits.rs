/// Bounds enforced on every document, whatever its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub blocks: usize,
    pub depth: usize,
    pub text_bytes: usize,
    pub runs: usize,
    pub props: usize,
    pub paste_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            blocks: 50_000,
            depth: 32,
            text_bytes: 1 << 20,
            runs: 10_000,
            props: 256,
            paste_bytes: 16 << 20,
        }
    }
}
