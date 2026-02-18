/// Fixed-capacity ring buffer. Oldest entry is overwritten when full.
#[derive(Debug)]
pub struct RingBuffer {
    data: Vec<u64>,
    head: usize,
    len: usize,
    cap: usize,
}

impl RingBuffer {
    pub fn new(cap: usize) -> Self {
        Self { data: vec![0; cap], head: 0, len: 0, cap }
    }

    pub fn push(&mut self, val: u64) {
        self.data[self.head] = val;
        self.head = (self.head + 1) % self.cap;
        if self.len < self.cap {
            self.len += 1;
        }
    }

    /// Returns up to `n` most-recent values, oldest first.
    pub fn last_n(&self, n: usize) -> Vec<u64> {
        let n = n.min(self.len);
        let mut out = Vec::with_capacity(n);
        for i in (0..n).rev() {
            let idx = (self.head + self.cap - 1 - i) % self.cap;
            out.push(self.data[idx]);
        }
        out.reverse();
        out
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
