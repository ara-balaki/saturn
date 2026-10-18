//! The design system's tokens as GPUI values, in the current appearance.

use gpui_kit::{BoxShadow, Hsla, Pixels, Rgba, point, px};
use saturn_design::{Appearance, Color, Shadow, Shadows, Token};

use crate::settings::AppearanceSetting;

pub fn color(token: Token) -> Hsla {
    rgba(token.get())
}

fn rgba(color: Color) -> Hsla {
    #[expect(clippy::cast_precision_loss, reason = "8-bit channels")]
    let channel = |shift: u32| ((color.rgb >> shift) & 0xff) as f32 / 255.0;
    Rgba { r: channel(16), g: channel(8), b: channel(0), a: color.alpha }.into()
}

pub fn shadows(token: Shadows) -> Vec<BoxShadow> {
    layers(token.get())
}

pub fn layers(layers: &[Shadow]) -> Vec<BoxShadow> {
    layers
        .iter()
        .map(|layer| BoxShadow {
            color: rgba(layer.color),
            offset: point(px(layer.x), px(layer.y)),
            blur_radius: px(layer.blur),
            spread_radius: px(layer.spread),
            inset: layer.inset,
        })
        .collect()
}

/// gpui-component's own controls (switches, dialogs, inputs, scrollbars) in Saturn's colours,
/// fonts and radii instead of the library's grey defaults, for one appearance.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "radii are small whole pixels")]
fn component_theme(appearance: Appearance) -> gpui_kit::component::ThemeConfig {
    use saturn_design::{color as c, font, radius};

    // `#rrggbb`, as the theme config takes colours as strings.
    let hex = |token: Token| {
        let color = match appearance {
            Appearance::Dark => token.dark,
            Appearance::Light => token.light,
        };
        format!("#{:06x}", color.rgb)
    };
    let colors = serde_json::json!({
        "background": hex(c::SURFACE),
        "foreground": hex(c::INK),
        "border": hex(c::LINE),
        "input.border": hex(c::LINE_STRONG),
        "ring": hex(c::PURPLE_TEXT),
        "muted.background": hex(c::CONTROL),
        "muted.foreground": hex(c::INK_MUTED),
        "accent.background": hex(c::CONTROL_HOVER),
        "accent.foreground": hex(c::INK),
        "primary.background": hex(c::PURPLE),
        "primary.hover.background": hex(c::PURPLE_DEEP),
        "primary.active.background": hex(c::PURPLE_DEEP),
        "primary.foreground": hex(c::INK_ON_ACCENT),
        "secondary.background": hex(c::CONTROL),
        "secondary.hover.background": hex(c::CONTROL_HOVER),
        "secondary.active.background": hex(c::CONTROL_PRESSED),
        "secondary.foreground": hex(c::INK),
        "popover.background": hex(c::SURFACE_RAISED),
        "popover.foreground": hex(c::INK),
        "overlay": if appearance == Appearance::Dark { "#00000099" } else { "#1a10302e" },
        "selection.background": format!("{}{}", hex(c::PURPLE), if appearance == Appearance::Dark { "8c" } else { "47" }),
        "caret": hex(c::INK),
        "switch.background": hex(c::CONTROL_PRESSED),
        "switch.thumb.background": hex(c::INK_ON_ACCENT),
        "scrollbar.background": "#00000000",
        "scrollbar.thumb.background": hex(c::LINE_STRONG),
        "scrollbar.thumb.hover.background": hex(c::INK_FAINT),
        "danger.background": hex(c::RED),
        "danger.foreground": hex(c::INK_ON_ACCENT),
    });
    serde_json::from_value(serde_json::json!({
        "name": "Saturn",
        "mode": if appearance == Appearance::Dark { "dark" } else { "light" },
        "font.family": font::SANS,
        "font.size": 13,
        "mono_font.family": font::MONO,
        "mono_font.size": 13,
        "radius": radius::RADIUS_MD as usize,
        "radius.lg": radius::RADIUS_2XL as usize,
        "shadow": false,
        "colors": colors,
    }))
    .expect("Saturn's theme config is valid")
}

/// Draws the whole app as `setting` says: Saturn's tokens, the library's controls, and the window
/// frame macOS draws (whose edge is pale in light mode and dark in dark mode).
pub fn apply(setting: AppearanceSetting, cx: &mut gpui_kit::App) {
    use gpui_kit::component::{Theme, ThemeMode};

    // Following the system means no override, so macOS reports its own appearance again.
    cx.set_window_appearance(match setting {
        AppearanceSetting::System => None,
        AppearanceSetting::Light => Some(gpui_kit::WindowAppearance::Light),
        AppearanceSetting::Dark => Some(gpui_kit::WindowAppearance::Dark),
    });
    let appearance = setting.resolve(system_appearance(cx.window_appearance()));
    saturn_design::set_appearance(appearance);
    Theme::update(cx, |theme| {
        theme.dark_theme = std::rc::Rc::new(component_theme(Appearance::Dark));
        theme.light_theme = std::rc::Rc::new(component_theme(Appearance::Light));
    });
    Theme::change(if appearance == Appearance::Dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    cx.refresh_windows();
}

/// The appearance macOS is in.
fn system_appearance(appearance: gpui_kit::WindowAppearance) -> Appearance {
    match appearance {
        gpui_kit::WindowAppearance::Light | gpui_kit::WindowAppearance::VibrantLight => Appearance::Light,
        gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark => Appearance::Dark,
    }
}

pub fn len(value: f32) -> Pixels {
    px(value)
}

/// A colour `t` of the way from `from` to `to`, blended in `OKLab` like the design system's
/// gradients, so orange to purple stays vivid through the middle.
#[expect(clippy::many_single_char_names, reason = "the OKLab formulas' own names")]
pub fn mix_oklab(from: Token, to: Token, t: f32) -> Hsla {
    let (from, to) = (from.get(), to.get());
    let (a, b) = (oklab(from), oklab(to));
    let mixed = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
    let [r, g, b] = srgb_from_oklab(mixed);
    Rgba { r, g, b, a: from.alpha + (to.alpha - from.alpha) * t }.into()
}

#[expect(clippy::many_single_char_names, reason = "the OKLab formulas' own names")]
fn oklab(token: Color) -> [f32; 3] {
    #[expect(clippy::cast_precision_loss, reason = "8-bit channels")]
    let linear = |shift: u32| {
        let c = ((token.rgb >> shift) & 0xff) as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (linear(16), linear(8), linear(0));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn srgb_from_oklab([l, a, b]: [f32; 3]) -> [f32; 3] {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let gamma = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
    };
    [
        gamma(4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_),
        gamma(-1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_),
        gamma(-0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_),
    ]
}

#[cfg(test)]
mod tests {
    use saturn_design::color;

    use super::*;

    #[test]
    fn an_oklab_mix_keeps_its_ends() {
        let start: Rgba = mix_oklab(color::ORANGE, color::PURPLE, 0.0).into();
        let end: Rgba = mix_oklab(color::ORANGE, color::PURPLE, 1.0).into();
        assert!((start.r - 1.0).abs() < 0.01 && (start.g - 144.0 / 255.0).abs() < 0.01);
        assert!((end.r - 114.0 / 255.0).abs() < 0.01 && (end.b - 235.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn an_oklab_midpoint_stays_brighter_than_an_srgb_one() {
        let middle: Rgba = mix_oklab(color::ORANGE, color::PURPLE, 0.5).into();
        // sRGB halfway is (184, 102, 141); OKLab keeps more light and saturation.
        assert!(middle.r + middle.g + middle.b > (184.0 + 102.0 + 141.0) / 255.0);
    }
}
