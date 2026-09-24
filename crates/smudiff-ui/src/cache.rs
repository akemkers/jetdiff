//! Cache for shapede linjer. Generisk over verdien så den kan testes uten GPUI.
//! Nøkkelen er det verdien avhenger av utenom linja (font, størrelse, farge, tekstversjon).
//! Endres nøkkelen tømmes alt.

use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug)]
pub struct LineCache<K, V> {
    key: Option<K>,
    lines: HashMap<usize, V>,
    capacity: usize,
}

impl<K: PartialEq, V> LineCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            key: None,
            lines: HashMap::new(),
            capacity,
        }
    }

    pub fn set_key(&mut self, key: K) {
        if self.key.as_ref() != Some(&key) {
            self.lines.clear();
            self.key = Some(key);
        }
    }

    pub fn get(&self, line: usize) -> Option<&V> {
        self.lines.get(&line)
    }

    pub fn get_or_insert_with(&mut self, line: usize, make: impl FnOnce() -> V) -> &V {
        self.lines.entry(line).or_insert_with(make)
    }

    /// Kalles etter en frame. Over kapasitet beholdes bare linjer nær det synlige, så
    /// en lang scroll gjennom en stor fil ikke holder på alt.
    pub fn trim(&mut self, visible: Range<usize>) {
        if self.lines.len() <= self.capacity {
            return;
        }
        let margin = self.capacity / 2;
        let keep = visible.start.saturating_sub(margin)..visible.end.saturating_add(margin);
        self.lines.retain(|line, _| keep.contains(line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lager_bare_en_gang() {
        let mut c: LineCache<u32, String> = LineCache::new(100);
        c.set_key(1);
        let mut calls = 0;
        for _ in 0..3 {
            c.get_or_insert_with(7, || {
                calls += 1;
                "x".into()
            });
        }
        assert_eq!(calls, 1);
        assert_eq!(c.lines.len(), 1);
    }

    #[test]
    fn ny_nokkel_tommer() {
        let mut c: LineCache<&str, u8> = LineCache::new(100);
        c.set_key("menlo 13");
        c.get_or_insert_with(1, || 1);
        c.set_key("menlo 13");
        assert_eq!(c.lines.len(), 1);
        c.set_key("jetbrains mono 13");
        assert!(c.lines.is_empty());
        assert_eq!(*c.get_or_insert_with(1, || 2), 2);
    }

    #[test]
    fn trim_beholder_naer_viewport() {
        let mut c: LineCache<(), usize> = LineCache::new(10);
        c.set_key(());
        for line in 0..20 {
            c.get_or_insert_with(line, || line);
        }
        c.trim(10..12);
        // Margin 5 rundt 10..12
        let mut kept: Vec<_> = c.lines.keys().copied().collect();
        kept.sort();
        assert_eq!(kept, (5..17).collect::<Vec<_>>());
        // Under kapasitet røres ingenting
        let mut c: LineCache<(), usize> = LineCache::new(10);
        for line in 0..5 {
            c.get_or_insert_with(line, || line);
        }
        c.trim(100..101);
        assert_eq!(c.lines.len(), 5);
    }
}
