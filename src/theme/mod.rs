pub mod builtin;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::Error as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Blend toward `other`: t=0 keeps self, t=1 gives other.
    pub fn blend(self, other: Rgb, t: f32) -> Rgb {
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
        )
    }

    pub fn parse(s: &str) -> Result<Rgb> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        match hex.len() {
            6 => {
                let n = u32::from_str_radix(hex, 16).context("invalid hex color")?;
                Ok(Rgb::new((n >> 16) as u8, (n >> 8) as u8, n as u8))
            }
            3 => {
                let n = u32::from_str_radix(hex, 16).context("invalid hex color")?;
                let (r, g, b) = ((n >> 8) & 0xf, (n >> 4) & 0xf, n & 0xf);
                Ok(Rgb::new((r * 17) as u8, (g * 17) as u8, (b * 17) as u8))
            }
            _ => bail!("invalid color {s:?}: expected #rrggbb or #rgb"),
        }
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Rgb, D::Error> {
        let s = String::deserialize(deserializer)?;
        Rgb::parse(&s).map_err(D::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
struct ThemeFile {
    name: String,
    colors: ColorsSection,
    #[serde(default)]
    chrome: ChromeSection,
}

#[derive(Debug, Deserialize)]
struct ColorsSection {
    foreground: Rgb,
    background: Rgb,
    black: Rgb,
    red: Rgb,
    green: Rgb,
    yellow: Rgb,
    blue: Rgb,
    magenta: Rgb,
    cyan: Rgb,
    white: Rgb,
    bright_black: Rgb,
    bright_red: Rgb,
    bright_green: Rgb,
    bright_yellow: Rgb,
    bright_blue: Rgb,
    bright_magenta: Rgb,
    bright_cyan: Rgb,
    bright_white: Rgb,
}

#[derive(Debug, Default, Deserialize)]
struct ChromeSection {
    title_fg: Option<Rgb>,
    shadow_opacity: Option<f32>,
    light_close: Option<Rgb>,
    light_minimize: Option<Rgb>,
    light_zoom: Option<Rgb>,
    button_fg: Option<Rgb>,
    button_bg: Option<Rgb>,
    bar_bg: Option<Rgb>,
    bar_fg: Option<Rgb>,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub foreground: Rgb,
    pub background: Rgb,
    pub palette: [Rgb; 16],
    pub title_fg: Rgb,
    pub shadow_opacity: f32,
    pub lights: [Rgb; 3],
    /// Caption glyph color for the windows/ubuntu chrome styles.
    pub button_fg: Rgb,
    /// Button disc fill for the ubuntu chrome style.
    pub button_bg: Rgb,
    /// Title bar fill for the windows/ubuntu chrome styles; None uses the
    /// style's authentic OS default.
    pub bar_bg: Option<Rgb>,
    /// Title text/glyph color on that bar; None uses the style default.
    pub bar_fg: Option<Rgb>,
}

impl Theme {
    pub fn from_toml(source: &str) -> Result<Theme> {
        let file: ThemeFile = toml::from_str(source).context("failed to parse theme")?;
        let c = &file.colors;
        let palette = [
            c.black,
            c.red,
            c.green,
            c.yellow,
            c.blue,
            c.magenta,
            c.cyan,
            c.white,
            c.bright_black,
            c.bright_red,
            c.bright_green,
            c.bright_yellow,
            c.bright_blue,
            c.bright_magenta,
            c.bright_cyan,
            c.bright_white,
        ];
        Ok(Self::build(
            file.name,
            c.foreground,
            c.background,
            palette,
            file.chrome,
        ))
    }

    /// A theme derived from bare colors with default chrome — for palettes
    /// embedded in recordings (asciicast v3 `term.theme`). An 8-entry
    /// palette reuses the normal colors as bright.
    pub fn from_palette(
        name: &str,
        foreground: Rgb,
        background: Rgb,
        palette: &[Rgb],
    ) -> Result<Theme> {
        let full: [Rgb; 16] = match palette.len() {
            16 => palette.try_into().unwrap(),
            8 => {
                let mut p = [Rgb::new(0, 0, 0); 16];
                p[..8].copy_from_slice(palette);
                p[8..].copy_from_slice(palette);
                p
            }
            n => bail!("palette has {n} colors; expected 8 or 16"),
        };
        Ok(Self::build(
            name.to_string(),
            foreground,
            background,
            full,
            ChromeSection::default(),
        ))
    }

    fn build(
        name: String,
        foreground: Rgb,
        background: Rgb,
        palette: [Rgb; 16],
        chrome: ChromeSection,
    ) -> Theme {
        let title_fg = chrome
            .title_fg
            .unwrap_or_else(|| foreground.blend(background, 0.45));
        Theme {
            name,
            foreground,
            background,
            palette,
            title_fg,
            shadow_opacity: chrome.shadow_opacity.unwrap_or(0.35),
            lights: [
                chrome.light_close.unwrap_or(Rgb::new(0xff, 0x5f, 0x57)),
                chrome.light_minimize.unwrap_or(Rgb::new(0xfe, 0xbc, 0x2e)),
                chrome.light_zoom.unwrap_or(Rgb::new(0x28, 0xc8, 0x40)),
            ],
            button_fg: chrome.button_fg.unwrap_or(title_fg),
            button_bg: chrome
                .button_bg
                .unwrap_or_else(|| title_fg.blend(background, 0.85)),
            bar_bg: chrome.bar_bg,
            bar_fg: chrome.bar_fg,
        }
    }

    pub fn resolve(&self, color: avt::Color) -> Rgb {
        match color {
            avt::Color::Indexed(i) => self.resolve_indexed(i),
            avt::Color::RGB(c) => Rgb::new(c.r, c.g, c.b),
        }
    }

    fn resolve_indexed(&self, i: u8) -> Rgb {
        match i {
            0..=15 => self.palette[i as usize],
            16..=231 => {
                let n = i - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
                Rgb::new(level(n / 36 % 6), level(n / 6 % 6), level(n % 6))
            }
            232..=255 => {
                let v = 8 + 10 * (i - 232);
                Rgb::new(v, v, v)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colors() {
        assert_eq!(Rgb::parse("#ff5555").unwrap(), Rgb::new(0xff, 0x55, 0x55));
        assert_eq!(Rgb::parse("fff").unwrap(), Rgb::new(0xff, 0xff, 0xff));
        assert!(Rgb::parse("#12345").is_err());
    }

    #[test]
    fn from_palette_derives_chrome_like_from_toml() {
        // A cast-embedded palette must derive the same chrome defaults a
        // TOML theme without a [chrome] section would.
        let toml_theme = Theme::from_toml(
            r##"
            name = "x"
            [colors]
            foreground = "#f8f8f2"
            background = "#282a36"
            black = "#000000"
            red = "#ff5555"
            green = "#50fa7b"
            yellow = "#f1fa8c"
            blue = "#bd93f9"
            magenta = "#ff79c6"
            cyan = "#8be9fd"
            white = "#bbbbbb"
            bright_black = "#44475a"
            bright_red = "#ff6e6e"
            bright_green = "#69ff94"
            bright_yellow = "#ffffa5"
            bright_blue = "#d6acff"
            bright_magenta = "#ff92df"
            bright_cyan = "#a4ffff"
            bright_white = "#ffffff"
            "##,
        )
        .unwrap();
        let cast_theme = Theme::from_palette(
            "x",
            toml_theme.foreground,
            toml_theme.background,
            &toml_theme.palette,
        )
        .unwrap();
        assert_eq!(cast_theme.palette, toml_theme.palette);
        assert_eq!(cast_theme.title_fg, toml_theme.title_fg);
        assert_eq!(cast_theme.button_bg, toml_theme.button_bg);
        assert_eq!(cast_theme.shadow_opacity, toml_theme.shadow_opacity);

        // 8 entries reuse the normal colors as bright; other lengths fail.
        let eight = &toml_theme.palette[..8];
        let expanded =
            Theme::from_palette("x", toml_theme.foreground, toml_theme.background, eight).unwrap();
        assert_eq!(expanded.palette[8..], expanded.palette[..8]);
        assert!(
            Theme::from_palette(
                "x",
                toml_theme.foreground,
                toml_theme.background,
                &eight[..5]
            )
            .is_err()
        );
    }

    #[test]
    fn resolves_256_color_cube() {
        let theme = builtin::load("dracula").unwrap();
        // 16 is black corner of the cube
        assert_eq!(theme.resolve(avt::Color::Indexed(16)), Rgb::new(0, 0, 0));
        // 231 is white corner
        assert_eq!(
            theme.resolve(avt::Color::Indexed(231)),
            Rgb::new(255, 255, 255)
        );
        // 196 is pure red (5,0,0): 55 + 40*5 = 255
        assert_eq!(theme.resolve(avt::Color::Indexed(196)), Rgb::new(255, 0, 0));
        // grayscale ramp
        assert_eq!(theme.resolve(avt::Color::Indexed(232)), Rgb::new(8, 8, 8));
        assert_eq!(
            theme.resolve(avt::Color::Indexed(255)),
            Rgb::new(238, 238, 238)
        );
    }
}
