pub mod assets;
pub mod subset;

/// Family name the embedded @font-face blocks declare.
pub const EMBEDDED_FAMILY: &str = "JetBrainsMono NFM";

/// Font-family chain when the subsetted font is embedded: the embedded face
/// first, close system fallbacks, then emoji fonts (emoji are deliberately
/// not subset — see subset.rs).
pub const EMBEDDED_FONT_STACK: &str = "'JetBrainsMono NFM',Menlo,Consolas,'DejaVu Sans Mono',monospace,'Apple Color Emoji','Segoe UI Emoji','Noto Color Emoji'";
