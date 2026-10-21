//! Innstillinger fra `$XDG_CONFIG_HOME/jetdiff/config.toml`, eller `~/.config` uten den.
//! Mangler fila, lages den fra malen i `config.toml`. Malen har også standardverdiene for
//! nøkler fila mangler, så eldre filer virker etter at nye innstillinger er lagt til.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use jetdiff_viewmodel::Theme;
use jetdiff_viewmodel::metrics::{MAX_FONT_SIZE, MIN_FONT_SIZE};
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use toml::{Table, Value};

const TEMPLATE: &str = include_str!("config.toml");

type NewTheme = fn() -> Theme;

const THEMES: [(&str, NewTheme); 3] = [
    ("catppuccin-mocha", Theme::mocha),
    ("jetbrains-dark", Theme::dark),
    ("jetbrains-light", Theme::light),
];

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(deserialize_with = "theme")]
    pub theme: Theme,
    #[serde(deserialize_with = "font_size")]
    pub font_size: f32,
    pub collapse_unchanged: bool,
    #[serde(rename = "view", deserialize_with = "view")]
    pub unified: bool,
    pub side_panel: SidePanel,
    pub terminal: Terminal,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidePanel {
    pub open: bool,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    pub icons: bool,
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
    let mut config: Table = TEMPLATE.parse().expect("malen er gyldig toml");
    merge(
        &mut config,
        text.parse().map_err(|e: toml::de::Error| e.to_string())?,
    );
    config.try_into().map_err(|e| e.to_string())
}

fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Table(base)), Value::Table(over)) => merge(base, over),
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn theme<'de, D: Deserializer<'de>>(d: D) -> Result<Theme, D::Error> {
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

fn font_size<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let size = f32::deserialize(d)?;
    if (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&size) {
        Ok(size)
    } else {
        let msg = format!("må være mellom {MIN_FONT_SIZE} og {MAX_FONT_SIZE}");
        Err(D::Error::custom(msg))
    }
}

fn view<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    match String::deserialize(d)?.as_str() {
        "split" => Ok(false),
        "unified" => Ok(true),
        _ => Err(D::Error::custom("må være split eller unified")),
    }
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

    const OK: &str = "theme = \"jetbrains-dark\"\nfont_size = 16\ncollapse_unchanged = false\nview = \"unified\"\n[side_panel]\nopen = true\n[terminal]\nicons = false\n";

    #[test]
    fn malen_er_gyldig() {
        let config = parse(TEMPLATE).unwrap();
        assert_eq!(config.theme, Theme::mocha());
        assert_eq!(config.font_size, 14.);
        assert!(config.collapse_unchanged);
        assert!(!config.unified);
        assert!(!config.side_panel.open);
        assert!(config.terminal.icons);
    }

    #[test]
    fn malen_nevner_alle_temaene() {
        for (name, _) in THEMES {
            assert!(TEMPLATE.contains(name), "{name} mangler i config.toml");
        }
    }

    #[test]
    fn leser_alle_innstillingene() {
        let config = parse(OK).unwrap();
        assert_eq!(config.theme, Theme::dark());
        assert_eq!(config.font_size, 16.);
        assert!(!config.collapse_unchanged);
        assert!(config.unified);
        assert!(config.side_panel.open);
        assert!(!config.terminal.icons);
    }

    #[test]
    fn ukjent_visning_er_feil() {
        let err = parse(&OK.replace("unified", "side-by-side")).unwrap_err();
        assert!(err.contains("må være split eller unified"), "{err}");
    }

    #[test]
    fn ukjent_tema_er_feil() {
        let err = parse(&OK.replace("jetbrains-dark", "nope")).unwrap_err();
        assert!(err.contains("må være en av catppuccin-mocha"), "{err}");
    }

    #[test]
    fn fontstorrelse_utenfor_grensene_er_feil() {
        let err = parse(&OK.replace("16", "41")).unwrap_err();
        assert!(err.contains("må være mellom 8 og 40"), "{err}");
        assert!(parse(&OK.replace("16", "7.5")).is_err());
    }

    #[test]
    fn ukjente_nokler_er_feil() {
        assert!(parse(&format!("tema = \"x\"\n{OK}")).is_err());
        assert!(parse(&format!("{OK}apen = true\n")).is_err());
    }

    #[test]
    fn manglende_nokler_hentes_fra_malen() {
        let config = parse("theme = \"jetbrains-dark\"\n[side_panel]\nopen = true\n").unwrap();
        assert_eq!(config.theme, Theme::dark());
        assert_eq!(config.font_size, 14.);
        assert!(config.collapse_unchanged);
        assert!(config.side_panel.open);
        assert_eq!(parse("").unwrap(), parse(TEMPLATE).unwrap());
    }
}
