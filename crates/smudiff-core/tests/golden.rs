//! Sammenligner `compare_lines`, `compare_lines_inner` og `compare_for_viewer` mot fasit fra
//! IntelliJ i `fixtures/*/expected.json`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use smudiff_core::{
    ComparisonPolicy, DiffTooBig, LineFragment, compare_for_viewer, compare_lines,
    compare_lines_inner,
};

const POLICIES: [(&str, ComparisonPolicy); 3] = [
    ("DEFAULT", ComparisonPolicy::Default),
    ("TRIM_WHITESPACES", ComparisonPolicy::TrimWhitespaces),
    ("IGNORE_WHITESPACES", ComparisonPolicy::IgnoreWhitespaces),
];

#[derive(Deserialize)]
struct Expected {
    policies: std::collections::HashMap<String, PolicyResult>,
}

#[derive(Deserialize)]
struct PolicyResult {
    lines: Vec<Fragment>,
    words: Vec<Fragment>,
    viewer: Vec<Fragment>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
struct Fragment {
    l1: [usize; 2],
    l2: [usize; 2],
    o1: [usize; 2],
    o2: [usize; 2],
    inner: Option<Vec<Inner>>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
struct Inner {
    o1: [usize; 2],
    o2: [usize; 2],
}

impl From<&LineFragment> for Fragment {
    fn from(f: &LineFragment) -> Self {
        Fragment {
            l1: [f.lines1.start, f.lines1.end],
            l2: [f.lines2.start, f.lines2.end],
            o1: [f.offsets1.start, f.offsets1.end],
            o2: [f.offsets2.start, f.offsets2.end],
            inner: f.inner.as_ref().map(|inner| {
                inner
                    .iter()
                    .map(|d| Inner {
                        o1: [d.offsets1.start, d.offsets1.end],
                        o2: [d.offsets2.start, d.offsets2.end],
                    })
                    .collect()
            }),
        }
    }
}

fn cases() -> Vec<PathBuf> {
    // SMUDIFF_FIXTURES lar oss kjøre mot et annet sett, f.eks. genererte stresstester
    let root = std::env::var_os("SMUDIFF_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"));
    let mut cases: Vec<PathBuf> = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("expected.json").is_file())
        .collect();
    cases.sort();
    cases
}

type Compare = fn(&str, &str, ComparisonPolicy) -> Result<Vec<LineFragment>, DiffTooBig>;

fn check(kind: &str, compare: Compare, expected_of: fn(&PolicyResult) -> &Vec<Fragment>) {
    let cases = cases();
    if cases.is_empty() {
        eprintln!("ingen fixtures med expected.json, golden-testen sjekker ingenting");
        return;
    }

    let mut failures = Vec::new();
    for (name, policy) in POLICIES {
        let mut passed = 0;
        for case in &cases {
            let a = fs::read_to_string(case.join("a.txt")).unwrap();
            let b = fs::read_to_string(case.join("b.txt")).unwrap();
            let expected: Expected =
                serde_json::from_str(&fs::read_to_string(case.join("expected.json")).unwrap())
                    .unwrap();
            let expected = expected_of(&expected.policies[name]);

            let actual: Vec<Fragment> = match compare(&a, &b, policy) {
                Ok(fragments) => fragments.iter().map(Fragment::from).collect(),
                Err(e) => {
                    failures.push(format!("{} {name}: {e:?}", case.display()));
                    continue;
                }
            };
            if &actual == expected {
                passed += 1;
            } else {
                failures.push(format!(
                    "{} {name}:\n  forventet {expected:?}\n  fikk      {actual:?}",
                    case.file_name().unwrap().to_string_lossy()
                ));
            }
        }
        eprintln!("{kind} {name}: {passed}/{} ok", cases.len());
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn lines_matcher_intellij() {
    check("lines", compare_lines, |p| &p.lines);
}

#[test]
fn words_matcher_intellij() {
    check("words", compare_lines_inner, |p| &p.words);
}

#[test]
fn viewer_matcher_intellij() {
    check("viewer", compare_for_viewer, |p| &p.viewer);
}
