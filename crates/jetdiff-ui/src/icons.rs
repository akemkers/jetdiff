//! File type icons and the branch icon from New UI in IntelliJ (`expui/fileTypes`, `expui/general/vcs`),
//! Apache 2.0. The Rust icon is from intellij-rust (MIT, see `icons/LICENSE-intellij-rust.txt`). Go
//! has an icon only in GoLand, so the Go icon is from Seti UI (MIT, see `icons/LICENSE-seti-ui.txt`).

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};
use jetdiff_viewmodel::Language;

macro_rules! icons {
    ($($name:literal),*) => {
        [$(
            (concat!($name, ".svg"), include_bytes!(concat!("../icons/", $name, ".svg")).as_slice()),
        )*]
    };
}

const ICONS: [(&str, &[u8]); 18] = icons!(
    "branch",
    "chevronDown",
    "chevronRight",
    "docker",
    "go",
    "java",
    "javaScript",
    "json",
    "kotlin",
    "markdown",
    "python",
    "rust",
    "sql",
    "text",
    "toml",
    "typeScript",
    "xml",
    "yaml"
);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ICONS.iter().map(|(name, _)| (*name).into()).collect())
    }
}

pub fn file_icon(path: &str) -> String {
    use Language::*;
    let name = match Language::from_path(path) {
        Some(Dockerfile) => "docker",
        Some(Java) => "java",
        Some(JavaScript) => "javaScript",
        Some(Json) => "json",
        Some(Kotlin) => "kotlin",
        Some(Markdown) => "markdown",
        Some(Python) => "python",
        Some(Sql) => "sql",
        Some(Toml) => "toml",
        Some(Tsx | TypeScript) => "typeScript",
        Some(Xml) => "xml",
        Some(Yaml) => "yaml",
        Some(Rust) => "rust",
        Some(Go) => "go",
        None => "text",
    };
    icon(name)
}

pub fn chevron_icon(open: bool) -> String {
    icon(if open { "chevronDown" } else { "chevronRight" })
}

pub fn branch_icon() -> String {
    icon("branch")
}

fn icon(name: &str) -> String {
    format!("{name}.svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_per_file_type() {
        assert_eq!(file_icon("src/A.kt"), "kotlin.svg");
        assert_eq!(file_icon("App.tsx"), "typeScript.svg");
        assert_eq!(file_icon("main.rs"), "rust.svg");
        assert_eq!(file_icon("main.go"), "go.svg");
        assert_eq!(file_icon("Makefile"), "text.svg");
        assert_eq!(branch_icon(), "branch.svg");
    }

    #[test]
    fn all_icons_exist() {
        for path in [
            "a.go",
            "a.java",
            "a.js",
            "a.json",
            "a.kt",
            "a.md",
            "a.py",
            "a.rs",
            "a.sql",
            "a.toml",
            "a.ts",
            "a.xml",
            "a.yml",
            "Dockerfile",
            "a",
        ] {
            assert!(Assets.load(&file_icon(path)).unwrap().is_some(), "{path}");
        }
        assert!(Assets.load(&branch_icon()).unwrap().is_some());
    }
}
