//! The Saturn design system's tokens as Rust constants, compiled from `design/tokens.json`.
//!
//! `color`, `space`, `radius`, `shadow` and `font` mirror the token families of the design system
//! (<https://claude.ai/artifact/RC4SBXYReNJprpjxDNapf3>). Change a value in `design/tokens.json`, never
//! here.
//!
//! Colours and shadows come in a dark and a light version; [`set_appearance`] picks which one
//! [`Token::get`] and [`Shadows::get`] return, for the whole app at once.

use std::sync::atomic::{AtomicBool, Ordering};

/// An sRGB colour as `0xRRGGBB` plus opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub rgb: u32,
    pub alpha: f32,
}

/// One layer of a glow or drop shadow, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    /// Drawn inside the element.
    pub inset: bool,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
}

static LIGHT: AtomicBool = AtomicBool::new(false);

/// Which version of every token the app draws with.
pub fn appearance() -> Appearance {
    if LIGHT.load(Ordering::Relaxed) { Appearance::Light } else { Appearance::Dark }
}

pub fn set_appearance(appearance: Appearance) {
    LIGHT.store(appearance == Appearance::Light, Ordering::Relaxed);
}

/// A colour token: its value in each theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token {
    pub dark: Color,
    pub light: Color,
}

impl Token {
    /// The value in the current appearance.
    pub fn get(self) -> Color {
        match appearance() {
            Appearance::Dark => self.dark,
            Appearance::Light => self.light,
        }
    }
}

/// A shadow token: its layers in each theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadows {
    pub dark: &'static [Shadow],
    pub light: &'static [Shadow],
}

impl Shadows {
    /// The layers in the current appearance.
    pub fn get(self) -> &'static [Shadow] {
        match appearance() {
            Appearance::Dark => self.dark,
            Appearance::Light => self.light,
        }
    }
}

// Generated colour values are hex as written in tokens.json, and the docs are the tokens' usage
// notes, which name components (IconTile) as the design system writes them.
#[allow(clippy::unreadable_literal, clippy::doc_markdown)]
mod generated {
    use super::{Color, Shadow, Shadows, Token};
    include!(concat!(env!("OUT_DIR"), "/tokens.rs"));
}
pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_colours_come_from_the_tokens() {
        assert_eq!(color::BLUE_VIVID.dark, Color { rgb: 0x001c_69e3, alpha: 1.0 });
        assert_eq!(color::BLUE_VIVID.light, color::BLUE_VIVID.dark);
        assert_eq!(color::STATUS_WORKING.dark, color::BLUE.dark);
    }

    #[test]
    fn light_mode_has_its_own_neutrals_and_signals() {
        assert_ne!(color::SURFACE.light, color::SURFACE.dark);
        // Amber text darkens in light mode; the amber fill stays, always with dark text on it.
        assert_ne!(color::AMBER_TEXT.light, color::AMBER.light);
        assert_eq!(color::AMBER.light, color::AMBER.dark);
        assert_eq!(color::STATUS_NEEDS_YOU.light, color::AMBER_TEXT.light);
        assert_eq!(color::INK_ON_ACCENT.light, color::INK_ON_ACCENT.dark);
    }

    /// Every row of the nesting table in the design system's README: inner = outer − padding.
    #[test]
    fn nested_radii_follow_the_padding_rule() {
        // The macOS window corner (about 16) and the 6px gap give panels their radius.
        let window = 16.0;
        let nestings = [
            (window, space::SPACE_2, radius::RADIUS_MD),
            (radius::RADIUS_MD, space::SPACE_1, radius::RADIUS_SM),
            (radius::RADIUS_LG, space::SPACE_3, radius::RADIUS_SM),
        ];
        for (outer, padding, inner) in nestings {
            assert!((outer - padding - inner).abs() < f32::EPSILON, "{outer} − {padding} ≠ {inner}");
        }
    }

    #[test]
    fn glows_have_a_ring_and_a_soft_halo() {
        for glow in [shadow::GLOW_NEEDS_YOU.dark, shadow::GLOW_NEEDS_YOU.light] {
            assert_eq!(glow.len(), 2);
            assert!((glow[0].spread - 1.0).abs() < f32::EPSILON);
            assert!(!glow[0].inset);
        }
        assert!((shadow::GLOW_NEEDS_YOU.dark[1].blur - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fonts_are_the_first_family_of_each_stack() {
        assert_eq!(font::SANS, "Urbanist");
        assert_eq!(font::MONO, "JetBrains Mono");
    }
}
