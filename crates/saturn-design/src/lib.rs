//! The Saturn design system's tokens as Rust constants, compiled from `design/tokens.json`.
//!
//! `color`, `space`, `radius`, `shadow` and `font` mirror the token families of the design system
//! (<https://claude.ai/artifact/RC4SBXYReNJprpjxDNapf3>). Change a value in `design/tokens.json`, never
//! here.

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
    pub color: Color,
}

// Generated colour values are hex as written in tokens.json.
#[allow(clippy::unreadable_literal)]
mod generated {
    use super::{Color, Shadow};
    include!(concat!(env!("OUT_DIR"), "/tokens.rs"));
}
pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_colours_come_from_the_tokens() {
        assert_eq!(color::PURPLE, Color { rgb: 0x0072_3ceb, alpha: 1.0 });
        assert_eq!(color::STATUS_NEEDS_YOU, color::YELLOW);
    }

    #[test]
    fn nested_radii_follow_the_padding_rule() {
        assert!((radius::RADIUS_LG - space::SPACE_1 - radius::RADIUS_MD).abs() < f32::EPSILON);
        assert!((radius::RADIUS_MD - space::SPACE_1 - radius::RADIUS_SM).abs() < f32::EPSILON);
    }

    #[test]
    fn glows_have_a_ring_and_a_soft_halo() {
        let glow = shadow::GLOW_NEEDS_YOU;
        assert_eq!(glow.len(), 2);
        assert!((glow[0].spread - 1.0).abs() < f32::EPSILON);
        assert!((glow[1].blur - 26.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fonts_are_the_first_family_of_each_stack() {
        assert_eq!(font::SANS, "Plus Jakarta Sans");
        assert_eq!(font::MONO, "JetBrains Mono");
    }
}
