//! Generated files, known by their name: lock files, bundles, source maps and snapshots. They are
//! listed for the reader, but sending them to the model only costs.

const NAMES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
    "Gemfile.lock",
    "poetry.lock",
    "uv.lock",
    "go.sum",
    "composer.lock",
    "gradle.lockfile",
];
const ENDINGS: &[&str] = &[".min.js", ".min.css", ".map", ".snap"];
const PARTS: &[&str] = &[".generated.", "/__snapshots__/", "/generated/"];

pub(super) fn is_generated(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let path = format!("/{path}");
    NAMES.contains(&name)
        || ENDINGS.iter().any(|ending| name.ends_with(ending))
        || PARTS.iter().any(|part| path.contains(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_files_bundles_and_snapshots() {
        assert!(is_generated("Cargo.lock"));
        assert!(is_generated("web/package-lock.json"));
        assert!(is_generated("dist/app.min.js"));
        assert!(is_generated("src/__snapshots__/view.test.ts.snap"));
        assert!(is_generated("api/client.generated.ts"));
        assert!(!is_generated("src/lock.rs"));
        assert!(!is_generated("src/generator.rs"));
    }
}
