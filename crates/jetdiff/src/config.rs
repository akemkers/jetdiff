//! Innstillinger fra `$XDG_CONFIG_HOME/jetdiff/config.toml`, eller `~/.config` uten den.
//! Mangler fila, lages den fra malen i `config.toml`, som også er standardverdiene.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use jetdiff_ui::UiTheme;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

const TEMPLATE: &str = include_str!("config.toml");

type NewTheme = fn() -> UiTheme;

const THEMES: [(&str, NewTheme); 3] = [
    ("catppuccin-mocha", UiTheme::mocha),
    ("jetbrains-dark", UiTheme::dark),
    ("jetbrains-light", UiTheme::light),
];

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(deserialize_with = "theme")]
    pub theme: UiTheme,
    pub folds: Folds,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Folds {
    /// Nye filer vises med alle folds åpne.
    pub open: bool,
}

pub fn load() -> Result<Config, String> {
    let Some(path) = path() else {
        return parse(TEMPLATE);
    };
    let text = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // Feiler det, brukes malen uten å lagres
            let _ = create(&path);
            Ok(TEMPLATE.to_owned())
        }
        read => read.map_err(|e| e.to_string()),
    };
    text.and_then(|t| parse(&t))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn parse(text: &str) -> Result<Config, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

fn theme<'de, D: Deserializer<'de>>(d: D) -> Result<UiTheme, D::Error> {
    let name = String::deserialize(d)?;
    THEMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, t)| t())
        .ok_or_else(|| {
            let names = THEMES.map(|(n, _)| n).join(", ");
            D::Error::custom(format!("må være en av {names}"))
        })
}

fn create(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, TEMPLATE)
}

fn path() -> Option<PathBuf> {
    // XDG-spesifikasjonen sier at relative stier skal ignoreres
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|d| d.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".config")))?;
    Some(dir.join("jetdiff/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malen_er_gyldig() {
        let config = parse(TEMPLATE).unwrap();
        assert_eq!(config.theme, UiTheme::mocha());
        assert!(!config.folds.open);
    }

    #[test]
    fn malen_nevner_alle_temaene() {
        for (name, _) in THEMES {
            assert!(TEMPLATE.contains(name), "{name} mangler i config.toml");
        }
    }

    #[test]
    fn leser_tema_og_folds() {
        let config = parse("theme = \"jetbrains-dark\"\n[folds]\nopen = true\n").unwrap();
        assert_eq!(config.theme, UiTheme::dark());
        assert!(config.folds.open);
    }

    #[test]
    fn ukjent_tema_er_feil() {
        let err = parse("theme = \"nope\"\n[folds]\nopen = true\n").unwrap_err();
        assert!(err.contains("må være en av catppuccin-mocha"), "{err}");
    }

    #[test]
    fn ukjente_og_manglende_nokler_er_feil() {
        let ok = "theme = \"jetbrains-dark\"\n[folds]\nopen = true\n";
        assert!(parse(&format!("tema = \"x\"\n{ok}")).is_err());
        assert!(parse(&format!("{ok}apen = true\n")).is_err());
        assert!(parse("theme = \"jetbrains-dark\"\n[folds]\n").is_err());
    }
}
