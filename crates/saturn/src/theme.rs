//! The design system's tokens as GPUI values.

use gpui_kit::{BoxShadow, Hsla, Pixels, Rgba, point, px};
use saturn_design::{Color, Shadow};

pub fn color(token: Color) -> Hsla {
    #[expect(clippy::cast_precision_loss, reason = "8-bit channels")]
    let channel = |shift: u32| ((token.rgb >> shift) & 0xff) as f32 / 255.0;
    Rgba { r: channel(16), g: channel(8), b: channel(0), a: token.alpha }.into()
}

pub fn shadows(layers: &[Shadow]) -> Vec<BoxShadow> {
    layers
        .iter()
        .map(|layer| BoxShadow {
            color: color(layer.color),
            offset: point(px(layer.x), px(layer.y)),
            blur_radius: px(layer.blur),
            spread_radius: px(layer.spread),
            inset: false,
        })
        .collect()
}

pub fn len(value: f32) -> Pixels {
    px(value)
}
