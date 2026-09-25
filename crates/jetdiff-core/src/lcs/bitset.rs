/// Enkel bitset med samme semantikk som IntelliJ sin: lesing utenfor lengden gir `false`.
#[derive(Debug, Clone)]
pub(crate) struct BitSet {
    bits: Vec<bool>,
}

impl BitSet {
    pub(crate) fn new(len: usize) -> Self {
        Self {
            bits: vec![false; len],
        }
    }

    pub(crate) fn get(&self, i: usize) -> bool {
        self.bits.get(i).copied().unwrap_or(false)
    }

    pub(crate) fn set(&mut self, i: usize, value: bool) {
        if i >= self.bits.len() {
            self.bits.resize(i + 1, false);
        }
        self.bits[i] = value;
    }

    pub(crate) fn set_range(&mut self, from: usize, to: usize, value: bool) {
        if from >= to {
            return;
        }
        if to > self.bits.len() {
            self.bits.resize(to, false);
        }
        self.bits[from..to].fill(value);
    }
}
