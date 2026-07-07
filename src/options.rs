//! The complete render request, decoupled from clap.
//!
//! One `RenderOptions` value describes everything about how a source
//! renders — theme, chrome, layout, fonts, and animation timing — without
//! saying where the source came from. It is the options type embedded in
//! SVG metadata for round-tripping and accepted as JSON by the wasm API,
//! so serialization must stay deterministic: fields serialize in
//! declaration order and unset values are skipped.

use serde::{Deserialize, Serialize};

use crate::render::{ChromeStyle, CursorStyle};

fn is_false(b: &bool) -> bool {
    !b
}

/// Keys mirror the long CLI flag names. Every field is optional so a
/// partial document (hand-written JSON, an old embedded block) still
/// deserializes; the effective-value accessors supply the CLI defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case", default)]
pub struct RenderOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_light: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_dark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chrome: Option<ChromeStyle>,
    #[serde(skip_serializing_if = "is_false")]
    pub no_window: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub no_background: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub no_shadow: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_emoji: Option<String>,
    /// Title fallback captured alongside ANSI input (the command string
    /// for PTY captures); outranked by OSC titles found in the bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_fallback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<f32>,
    #[serde(skip_serializing_if = "is_false")]
    pub no_font_embed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_time_limit: Option<f64>,
    #[serde(skip_serializing_if = "is_false")]
    pub no_loop: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<f64>,
    #[serde(rename = "static", skip_serializing_if = "is_false")]
    pub static_: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<f64>,
}

impl RenderOptions {
    pub fn theme(&self) -> &str {
        self.theme.as_deref().unwrap_or("dracula")
    }

    pub fn font_size(&self) -> f32 {
        self.font_size.unwrap_or(14.0)
    }

    pub fn line_height(&self) -> f32 {
        self.line_height.unwrap_or(1.2)
    }

    pub fn padding(&self) -> f32 {
        self.padding.unwrap_or(10.0)
    }

    pub fn cols(&self) -> usize {
        self.cols.unwrap_or(80)
    }

    pub fn rows(&self) -> usize {
        self.rows.unwrap_or(24)
    }

    pub fn speed(&self) -> f64 {
        self.speed.unwrap_or(1.0)
    }

    pub fn cursor(&self) -> CursorStyle {
        self.cursor.unwrap_or_default()
    }

    /// The effective chrome style: --no-window and --no-background both
    /// force a bare panel.
    pub fn chrome(&self) -> ChromeStyle {
        if self.no_window || self.no_background {
            ChromeStyle::None
        } else {
            self.chrome.unwrap_or_default()
        }
    }

    /// --no-background leaves nothing to cast a shadow.
    pub fn shadow(&self) -> bool {
        !self.no_shadow && !self.no_background
    }

    pub fn margin(&self) -> f32 {
        self.margin
            .unwrap_or(if self.shadow() { 24.0 } else { 0.0 })
    }

    /// Both dual-theme names, when the pair is requested.
    pub fn dual_themes(&self) -> Option<(&str, &str)> {
        match (self.theme_light.as_deref(), self.theme_dark.as_deref()) {
            (Some(l), Some(d)) => Some((l, d)),
            _ => None,
        }
    }

    pub fn is_static(&self) -> bool {
        self.static_ || self.at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_mirror_the_cli() {
        let o = RenderOptions::default();
        assert_eq!(o.theme(), "dracula");
        assert_eq!(o.font_size(), 14.0);
        assert_eq!(o.line_height(), 1.2);
        assert_eq!(o.padding(), 10.0);
        assert_eq!((o.cols(), o.rows()), (80, 24));
        assert_eq!(o.speed(), 1.0);
        assert_eq!(o.chrome(), ChromeStyle::Macos);
        assert!(o.shadow());
        assert_eq!(o.margin(), 24.0);
        assert_eq!(o.cursor(), CursorStyle::Block);
        assert!(!o.is_static());
    }

    #[test]
    fn overrides_interact_like_the_cli() {
        let o = RenderOptions {
            no_background: true,
            chrome: Some(ChromeStyle::Ubuntu),
            ..Default::default()
        };
        assert_eq!(o.chrome(), ChromeStyle::None);
        assert!(!o.shadow());
        assert_eq!(o.margin(), 0.0);

        let o = RenderOptions {
            at: Some(2.5),
            ..Default::default()
        };
        assert!(o.is_static());
    }

    #[test]
    fn serialization_is_compact_and_round_trips() {
        let o = RenderOptions {
            theme: Some("nord".into()),
            no_shadow: true,
            speed: Some(2.0),
            static_: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&o).unwrap();
        assert_eq!(
            json,
            r#"{"theme":"nord","no-shadow":true,"speed":2.0,"static":true}"#
        );
        let back: RenderOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back, o);

        // Unset fields are skipped entirely; empty options are "{}".
        assert_eq!(
            serde_json::to_string(&RenderOptions::default()).unwrap(),
            "{}"
        );
    }

    #[test]
    fn unknown_keys_fail_loudly() {
        assert!(serde_json::from_str::<RenderOptions>(r#"{"font-szie":16}"#).is_err());
    }
}
