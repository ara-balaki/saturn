//! Terminal colours from the design system: the ANSI palette is the `term-*` tokens, so programs'
//! output matches Saturn's chrome.

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use saturn_design::color;

#[expect(clippy::cast_possible_truncation, reason = "taking the bytes of 0xRRGGBB")]
const fn rgb(token: saturn_design::Color) -> Rgb {
    Rgb { r: (token.rgb >> 16) as u8, g: (token.rgb >> 8) as u8, b: token.rgb as u8 }
}

pub const FOREGROUND: Rgb = rgb(color::TERM_WHITE);
pub const BACKGROUND: Rgb = rgb(color::SURFACE);
pub const CURSOR: Rgb = rgb(color::INK);
pub const SELECTION: Rgb = rgb(color::PURPLE);

/// ANSI 0–15: the eight `term-*` colours, then their bright versions.
const ANSI: [Rgb; 16] = [
    rgb(color::TERM_BLACK),
    rgb(color::TERM_RED),
    rgb(color::TERM_GREEN),
    rgb(color::TERM_YELLOW),
    rgb(color::TERM_BLUE),
    rgb(color::TERM_MAGENTA),
    rgb(color::TERM_CYAN),
    rgb(color::TERM_WHITE),
    rgb(color::INK_FAINT),
    rgb(color::TERM_RED),
    rgb(color::TERM_GREEN),
    rgb(color::TERM_YELLOW),
    rgb(color::TERM_BLUE),
    rgb(color::TERM_MAGENTA),
    rgb(color::TERM_CYAN),
    rgb(color::INK),
];

/// The xterm 256-colour palette: ANSI, a 6×6×6 cube, then 24 greys.
fn indexed(index: u8) -> Rgb {
    match index {
        0..=15 => ANSI[usize::from(index)],
        16..=231 => {
            let level = |value: u8| if value == 0 { 0 } else { 55 + value * 40 };
            let cube = index - 16;
            Rgb { r: level(cube / 36), g: level((cube / 6) % 6), b: level(cube % 6) }
        }
        _ => {
            let grey = 8 + (index - 232) * 10;
            Rgb { r: grey, g: grey, b: grey }
        }
    }
}

fn dim(color: Rgb) -> Rgb {
    let scale = |channel: u8| u8::try_from(u16::from(channel) * 2 / 3).unwrap_or(channel);
    Rgb { r: scale(color.r), g: scale(color.g), b: scale(color.b) }
}

/// Resolves a cell colour. Colours a program set itself (OSC 4/10/11) win over the palette.
pub fn resolve(color: Color, overrides: &Colors) -> Rgb {
    match color {
        Color::Spec(rgb) => rgb,
        Color::Indexed(index) => overrides[usize::from(index)].unwrap_or_else(|| indexed(index)),
        Color::Named(name) => overrides[name].unwrap_or_else(|| named(name)),
    }
}

fn named(name: NamedColor) -> Rgb {
    match name {
        NamedColor::Foreground | NamedColor::BrightForeground => FOREGROUND,
        NamedColor::Background => BACKGROUND,
        NamedColor::Cursor => CURSOR,
        NamedColor::DimForeground => dim(FOREGROUND),
        NamedColor::DimBlack => dim(ANSI[0]),
        NamedColor::DimRed => dim(ANSI[1]),
        NamedColor::DimGreen => dim(ANSI[2]),
        NamedColor::DimYellow => dim(ANSI[3]),
        NamedColor::DimBlue => dim(ANSI[4]),
        NamedColor::DimMagenta => dim(ANSI[5]),
        NamedColor::DimCyan => dim(ANSI[6]),
        NamedColor::DimWhite => dim(ANSI[7]),
        other => ANSI[(other as usize).min(15)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_colours_are_the_design_tokens() {
        assert_eq!(indexed(1), rgb(color::TERM_RED));
        assert_eq!(indexed(15), rgb(color::INK));
    }

    #[test]
    fn the_cube_and_greys_match_xterm() {
        assert_eq!(indexed(16), Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(indexed(196), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(indexed(232), Rgb { r: 8, g: 8, b: 8 });
        assert_eq!(indexed(255), Rgb { r: 238, g: 238, b: 238 });
    }
}
