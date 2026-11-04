//! Settings from `$XDG_CONFIG_HOME/jetdiff/config.toml`, or `~/.config` without it.
//! If the file is missing, it is made from the template in `config.toml`. The template also has the defaults for
//! keys the file is missing, so older files work after new settings are added.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use jetdiff_viewmodel::metrics::{MAX_FONT_SIZE, MIN_FONT_SIZE};
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use toml::{Table, Value};

const TEMPLATE: &str = include_str!("config.toml");

/// Settings that are gone, ignored so older files still load.
const REMOVED_KEYS: [&str; 1] = ["theme"];

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(deserialize_with = "font_size")]
    pub font_size: f32,
    pub fold_unchanged: bool,
    #[serde(rename = "diff_mode", deserialize_with = "diff_mode")]
    pub unified: bool,
    pub walkthrough: Walkthrough,
    pub side_panel: SidePanel,
    pub terminal: Terminal,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Walkthrough {
    pub command: Vec<String>,
    pub model: String,
    pub language: String,
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
            // If that fails, the template is used without being saved
            let _ = create(&path);
            Ok(TEMPLATE.to_owned())
        }
        read => read.map_err(|e| e.to_string()),
    };
    text.and_then(|t| parse(&t))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn parse(text: &str) -> Result<Config, String> {
    let mut config: Table = TEMPLATE.parse().expect("the template is valid toml");
    merge(
        &mut config,
        text.parse().map_err(|e: toml::de::Error| e.to_string())?,
    );
    for key in REMOVED_KEYS {
        config.remove(key);
    }
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

fn font_size<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let size = f32::deserialize(d)?;
    if (MIN_FONT_SIZE..=MAX_FONT_SIZE).contains(&size) {
        Ok(size)
    } else {
        let msg = format!("must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE}");
        Err(D::Error::custom(msg))
    }
}

fn diff_mode<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    match String::deserialize(deserializer)?.as_str() {
        "split" => Ok(false),
        "unified" => Ok(true),
        _ => Err(D::Error::custom("must be split or unified")),
    }
}

fn create(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, TEMPLATE)
}

fn path() -> Option<PathBuf> {
    // The XDG specification says relative paths should be ignored
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|d| d.is_absolute())
        .or_else(|| std::env::home_dir().map(|h| h.join(".config")))?;
    Some(dir.join("jetdiff/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: &str = "font_size = 16\nfold_unchanged = false\ndiff_mode = \"unified\"\n[side_panel]\nopen = true\n[terminal]\nicons = false\n";

    #[test]
    fn template_is_valid() {
        let config = parse(TEMPLATE).unwrap();
        assert_eq!(config.font_size, 14.);
        assert!(config.fold_unchanged);
        assert!(!config.unified);
        assert!(!config.side_panel.open);
        assert!(config.terminal.icons);
    }

    #[test]
    fn reads_all_settings() {
        let config = parse(OK).unwrap();
        assert_eq!(config.font_size, 16.);
        assert!(!config.fold_unchanged);
        assert!(config.unified);
        assert!(config.side_panel.open);
        assert!(!config.terminal.icons);
    }

    #[test]
    fn unknown_diff_mode_is_an_error() {
        let err = parse(&OK.replace("unified", "side-by-side")).unwrap_err();
        assert!(err.contains("must be split or unified"), "{err}");
    }

    #[test]
    fn an_old_theme_is_ignored() {
        let config = parse(&format!("theme = \"jetbrains-light\"\n{OK}")).unwrap();
        assert_eq!(config, parse(OK).unwrap());
    }

    #[test]
    fn font_size_outside_limits_is_an_error() {
        let err = parse(&OK.replace("16", "41")).unwrap_err();
        assert!(err.contains("must be between 8 and 40"), "{err}");
        assert!(parse(&OK.replace("16", "7.5")).is_err());
    }

    #[test]
    fn unknown_keys_are_an_error() {
        assert!(parse(&format!("color_theme = \"x\"\n{OK}")).is_err());
        assert!(parse(&format!("{OK}is_open = true\n")).is_err());
    }

    #[test]
    fn missing_keys_come_from_the_template() {
        let config = parse("[side_panel]\nopen = true\n").unwrap();
        assert_eq!(config.font_size, 14.);
        assert!(config.fold_unchanged);
        assert!(config.side_panel.open);
        assert_eq!(parse("").unwrap(), parse(TEMPLATE).unwrap());
    }
}
