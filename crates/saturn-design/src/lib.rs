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
    /// Drawn inside the element, like the top highlight of `gloss` and `lift`.
    pub inset: bool,
    pub color: Color,
}

// Generated colour values are hex as written in tokens.json, and the docs are the tokens' usage
// notes, which name components (IconTile) as the design system writes them.
#[allow(clippy::unreadable_literal, clippy::doc_markdown)]
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
    fn gloss_lights_the_top_edge_from_inside() {
        let top = shadow::GLOSS[0];
        assert!(top.inset);
        assert!((top.y - 1.0).abs() < f32::EPSILON);
        assert!(!shadow::GLOW_FOCUS[0].inset);
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
