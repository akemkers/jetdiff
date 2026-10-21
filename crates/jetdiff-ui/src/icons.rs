//! Filtype-ikoner fra New UI i IntelliJ (`expui/fileTypes`), Apache 2.0. Rust-ikonet er fra
//! intellij-rust (MIT, se `icons/LICENSE-intellij-rust.txt`). Go har ikon bare i GoLand, så
//! Go-ikonet er fra Seti UI (MIT, se `icons/LICENSE-seti-ui.txt`).

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};
use jetdiff_viewmodel::Language;

macro_rules! icons {
    ($($name:literal),*) => {
        [$(
            (concat!($name, ".svg"), include_bytes!(concat!("../icons/", $name, ".svg")).as_slice()),
            (concat!($name, "_dark.svg"), include_bytes!(concat!("../icons/", $name, "_dark.svg")).as_slice()),
        )*]
    };
}

const ICONS: [(&str, &[u8]); 30] = icons!(
    "chevronDown",
    "chevronRight",
    "folder",
    "go",
    "java",
    "javaScript",
    "json",
    "kotlin",
    "markdown",
    "rust",
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

pub fn file_icon(path: &str, dark: bool) -> String {
    use Language::*;
    let name = match Language::from_path(path) {
        Some(Java) => "java",
        Some(JavaScript) => "javaScript",
        Some(Json) => "json",
        Some(Kotlin) => "kotlin",
        Some(Markdown) => "markdown",
        Some(Toml) => "toml",
        Some(Tsx | TypeScript) => "typeScript",
        Some(Xml) => "xml",
        Some(Yaml) => "yaml",
        Some(Rust) => "rust",
        Some(Go) => "go",
        None => "text",
    };
    icon(name, dark)
}

pub fn chevron_icon(expanded: bool, dark: bool) -> String {
    icon(
        if expanded {
            "chevronDown"
        } else {
            "chevronRight"
        },
        dark,
    )
}

pub fn folder_icon(dark: bool) -> String {
    icon("folder", dark)
}

fn icon(name: &str, dark: bool) -> String {
    let suffix = if dark { "_dark" } else { "" };
    format!("{name}{suffix}.svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ikon_per_filtype() {
        assert_eq!(file_icon("src/A.kt", true), "kotlin_dark.svg");
        assert_eq!(file_icon("App.tsx", false), "typeScript.svg");
        assert_eq!(file_icon("main.rs", false), "rust.svg");
        assert_eq!(file_icon("main.go", false), "go.svg");
        assert_eq!(file_icon("Makefile", true), "text_dark.svg");
        assert_eq!(folder_icon(true), "folder_dark.svg");
    }

    #[test]
    fn alle_ikoner_finnes() {
        for path in [
            "a.go", "a.java", "a.js", "a.json", "a.kt", "a.md", "a.rs", "a.toml", "a.ts", "a.xml",
            "a.yml", "a",
        ] {
            for dark in [false, true] {
                assert!(
                    Assets.load(&file_icon(path, dark)).unwrap().is_some(),
                    "{path}"
                );
            }
        }
    }
}
