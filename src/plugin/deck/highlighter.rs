use std::io::Cursor;
use std::sync::OnceLock;

use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span, Text};
use syntect::dumps;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

/// Initialize the syntax set and theme using prebuilt data
fn init() -> (&'static Theme, &'static SyntaxSet) {
    let theme = THEME.get_or_init(|| {
        let theme_bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/preset/themes/dracula.tmTheme"
        ));
        ThemeSet::load_from_reader(&mut Cursor::new(theme_bytes)).expect("Failed to load theme")
    });
    //
    let syntaxes = SYNTAXES.get_or_init(|| {
        let syntaxes_bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/preset/syntaxes/syntaxes"
        ));
        dumps::from_uncompressed_data(syntaxes_bytes).expect("Failed to load syntaxes")
    });
    //
    (theme, syntaxes)
}

/// Find a syntax definition by language name or file extension
fn find_syntax(name: &str) -> Option<&'static syntect::parsing::SyntaxReference> {
    let (_, syntaxes) = init();
    // Try extension first
    if let Some(syntax) = syntaxes.find_syntax_by_extension(name) {
        return Some(syntax);
    }
    // Fall back to language token/name search
    syntaxes.find_syntax_by_token(name)
}

/// Convert syntect color to ratatui Color
fn syntect_color_to_ratatui(color: syntect::highlighting::Color) -> Option<Color> {
    if color.a == 0 {
        // Theme color (from 8-bit palette)
        match color.r {
            0x00 => Some(Color::Black),
            0x01 => Some(Color::Red),
            0x02 => Some(Color::Green),
            0x03 => Some(Color::Yellow),
            0x04 => Some(Color::Blue),
            0x05 => Some(Color::Magenta),
            0x06 => Some(Color::Cyan),
            0x07 => Some(Color::White),
            n => Some(Color::Indexed(n)),
        }
    } else if color.a == 1 {
        None // Use default color
    } else {
        Some(Color::Rgb(color.r, color.g, color.b))
    }
}

/// Convert syntect font style to ratatui modifier
fn syntect_font_style_to_modifier(font_style: syntect::highlighting::FontStyle) -> Modifier {
    let mut modifier = Modifier::empty();
    if font_style.contains(syntect::highlighting::FontStyle::BOLD) {
        modifier |= Modifier::BOLD;
    }
    if font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
        modifier |= Modifier::ITALIC;
    }
    if font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
        modifier |= Modifier::UNDERLINED;
    }
    modifier
}

/// Highlight a code string and convert to TUI Text
pub fn highlight(code: &str, language: &str) -> Result<Text<'static>, String> {
    let (theme, syntaxes) = init();

    // Find syntax for the language, fall back to plain text if not found
    let syntax = find_syntax(language).unwrap_or_else(|| {
        syntaxes
            .find_syntax_by_name("Plain Text")
            .expect("Plain Text syntax should always exist")
    });

    let mut highlighter = HighlightLines::new(syntax, theme);

    let mut lines = Vec::new();

    for line_str in LinesWithEndings::from(code) {
        match highlighter.highlight_line(line_str, syntaxes) {
            Ok(regions) => {
                let spans: Vec<Span> = regions
                    .into_iter()
                    .map(|(style, text)| {
                        let fg = syntect_color_to_ratatui(style.foreground);
                        let modifier = syntect_font_style_to_modifier(style.font_style);

                        Span {
                            content: text.replace('\t', "  ").into(),
                            style: ratatui::style::Style {
                                fg,
                                add_modifier: modifier,
                                ..Default::default()
                            },
                        }
                    })
                    .collect();
                lines.push(Line::from(spans));
            }
            Err(e) => {
                lines.push(Line::raw(e.to_string()));
            }
        }
    }

    Ok(Text::from(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_javascript() {
        let code = r#"
function hello(name) {
    console.log("Hello, " + name);
    return true;
}
"#;
        let result = highlight(code, "javascript");
        assert!(result.is_ok());
    }

    #[test]
    fn test_highlight_rust() {
        let code = r#"
fn main() {
    println!("Hello, world!");
}
"#;
        let result = highlight(code, "rust");
        assert!(result.is_ok());
    }

    #[test]
    fn test_highlight_unknown_language() {
        let code = "some code";
        // Should still work with plain text
        let result = highlight(code, "unknownlanguage");
        assert!(result.is_ok());
    }
}
