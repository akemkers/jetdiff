use super::*;

fn ranges(a: &str, b: &str) -> Vec<(usize, usize, usize, usize)> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let changes = diff(&a, &b).unwrap();
    assert_valid(&a, &b, &changes);
    changes
        .ranges
        .iter()
        .map(|r| (r.start1, r.end1, r.start2, r.end2))
        .collect()
}

fn assert_valid<T: Eq + std::fmt::Debug>(a: &[T], b: &[T], changes: &Changes) {
    assert_eq!((changes.len1, changes.len2), (a.len(), b.len()));
    let mut last = (0, 0);
    for r in &changes.ranges {
        assert!(
            r.start1 >= last.0 && r.start2 >= last.1,
            "overlap or wrong order: {changes:?}"
        );
        assert!(!r.is_empty(), "empty change: {changes:?}");
        last = (r.end1, r.end2);
    }
    for u in changes.unchanged() {
        assert_eq!(u.end1 - u.start1, u.end2 - u.start2, "unchanged is not 1:1");
        assert_eq!(
            a[u.start1..u.end1],
            b[u.start2..u.end2],
            "unchanged is not equal"
        );
    }
}

fn lcs_len<T: Eq>(a: &[T], b: &[T]) -> usize {
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..a.len() {
        for j in 0..b.len() {
            dp[i + 1][j + 1] = if a[i] == b[j] {
                dp[i][j] + 1
            } else {
                dp[i][j + 1].max(dp[i + 1][j])
            };
        }
    }
    dp[a.len()][b.len()]
}

#[test]
fn equal_sequences_give_no_changes() {
    assert_eq!(ranges("abc", "abc"), vec![]);
    assert_eq!(ranges("", ""), vec![]);
}

#[test]
fn pure_insert_and_delete() {
    assert_eq!(ranges("ac", "abc"), vec![(1, 1, 1, 2)]);
    assert_eq!(ranges("abc", "ac"), vec![(1, 2, 1, 1)]);
    assert_eq!(ranges("", "ab"), vec![(0, 0, 0, 2)]);
    assert_eq!(ranges("ab", ""), vec![(0, 2, 0, 0)]);
}

#[test]
fn replacement_in_the_middle() {
    assert_eq!(ranges("axc", "ayc"), vec![(1, 2, 1, 2)]);
}

#[test]
fn unique_elements_are_discarded_by_reindexer() {
    // x and y exist on one side only, so they can never be matched.
    assert_eq!(ranges("axbc", "abyc"), vec![(1, 2, 1, 1), (3, 3, 2, 3)]);
}

#[test]
fn completely_different_sequences() {
    assert_eq!(ranges("abc", "xyz"), vec![(0, 3, 0, 3)]);
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn seq(&mut self, max_len: u64, alphabet: u64) -> Vec<u32> {
        let len = self.next() % max_len;
        (0..len).map(|_| (self.next() % alphabet) as u32).collect()
    }
}

#[test]
fn myers_gives_optimal_lcs_on_random_input() {
    let mut rng = Rng(0x5eed);
    for _ in 0..3000 {
        let alphabet = 1 + rng.next() % 8;
        let a = rng.seq(40, alphabet);
        let b = rng.seq(40, alphabet);
        let changes = diff_ints(&a, &b).unwrap();
        assert_valid(&a, &b, &changes);
        let equal: usize = changes.unchanged().map(|u| u.end1 - u.start1).sum();
        assert_eq!(equal, lcs_len(&a, &b), "not optimal for a={a:?} b={b:?}");
    }
}

#[test]
fn patience_is_valid_on_random_input() {
    let mut rng = Rng(0xfeed);
    for _ in 0..3000 {
        let alphabet = 1 + rng.next() % 30;
        let a = rng.seq(60, alphabet);
        let b = rng.seq(60, alphabet);
        let mut changes = [BitSet::new(a.len()), BitSet::new(b.len())];
        let [c1, c2] = &mut changes;
        PatienceLcs::new(&a, &b, c1, c2).execute(false).unwrap();
        // Unchanged elements must be paired 1:1 and be equal.
        let kept1: Vec<u32> = (0..a.len())
            .filter(|&i| !changes[0].get(i))
            .map(|i| a[i])
            .collect();
        let kept2: Vec<u32> = (0..b.len())
            .filter(|&i| !changes[1].get(i))
            .map(|i| b[i])
            .collect();
        assert_eq!(kept1, kept2, "a={a:?} b={b:?}");
    }
}

#[test]
fn large_input_without_common_elements() {
    let a: Vec<u32> = (0..50_000).collect();
    let b: Vec<u32> = (50_000..100_000).collect();
    assert_eq!(
        diff_ints(&a, &b).unwrap().ranges,
        vec![Range::new(0, 50_000, 0, 50_000)]
    );
}
