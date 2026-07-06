use anyhow::{Context, Result, bail};
use std::path::Path;

use super::Theme;

pub const BUILTIN: &[(&str, &str)] = &[
    ("dracula", include_str!("../../themes/dracula.toml")),
    (
        "catppuccin-mocha",
        include_str!("../../themes/catppuccin-mocha.toml"),
    ),
    ("nord", include_str!("../../themes/nord.toml")),
    ("tokyo-night", include_str!("../../themes/tokyo-night.toml")),
    ("github-dark", include_str!("../../themes/github-dark.toml")),
    (
        "github-light",
        include_str!("../../themes/github-light.toml"),
    ),
    (
        "solarized-dark",
        include_str!("../../themes/solarized-dark.toml"),
    ),
    ("powershell", include_str!("../../themes/powershell.toml")),
    ("ubuntu", include_str!("../../themes/ubuntu.toml")),
];

pub fn names() -> impl Iterator<Item = &'static str> {
    BUILTIN.iter().map(|(name, _)| *name)
}

/// Load a theme: a built-in name, or a path to a custom .toml file.
pub fn load(name_or_path: &str) -> Result<Theme> {
    if let Some((_, source)) = BUILTIN.iter().find(|(name, _)| *name == name_or_path) {
        return Theme::from_toml(source);
    }
    if name_or_path.contains('/') || name_or_path.ends_with(".toml") {
        let source = std::fs::read_to_string(Path::new(name_or_path))
            .with_context(|| format!("failed to read theme file {name_or_path}"))?;
        return Theme::from_toml(&source);
    }
    bail!(
        "unknown theme {name_or_path:?} (available: {}; or pass a path to a .toml file)",
        names().collect::<Vec<_>>().join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_themes_parse() {
        for (name, _) in BUILTIN {
            let theme = load(name).unwrap_or_else(|e| panic!("theme {name} failed: {e}"));
            assert_eq!(&theme.name, name, "name field must match registry key");
        }
    }
}
