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
        assert_eq!(color::PURPLE.dark, Color { rgb: 0x0072_3ceb, alpha: 1.0 });
        assert_eq!(color::PURPLE.light, color::PURPLE.dark);
        assert_eq!(color::STATUS_NEEDS_YOU.dark, color::YELLOW.dark);
    }

    #[test]
    fn light_mode_has_its_own_neutrals_and_signals() {
        assert_ne!(color::SURFACE.light, color::SURFACE.dark);
        // Yellow is invisible on white except as a fill, so needs-you text turns amber.
        assert_ne!(color::STATUS_NEEDS_YOU.light, color::YELLOW.light);
        // Text on the brand colours stays the same in both themes.
        assert_eq!(color::INK_ON_ACCENT.light, color::INK_ON_ACCENT.dark);
    }

    /// Every row of the nesting table in the design system's README: inner = outer − padding.
    #[test]
    fn nested_radii_follow_the_padding_rule() {
        let nestings = [
            (radius::RADIUS_XL, space::SPACE_2, radius::RADIUS_LG),
            (radius::RADIUS_LG, space::SPACE_1, radius::RADIUS_MD),
            (radius::RADIUS_MD, space::SPACE_1, radius::RADIUS_SM),
            (radius::RADIUS_2XL, 14.0, radius::RADIUS_LG),
            (radius::RADIUS_2XL, 18.0, radius::RADIUS_MD),
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
        assert!((shadow::GLOW_NEEDS_YOU.dark[1].blur - 26.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fonts_are_the_first_family_of_each_stack() {
        assert_eq!(font::SANS, "Plus Jakarta Sans");
        assert_eq!(font::MONO, "JetBrains Mono");
    }
}
