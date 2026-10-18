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
        "ring": hex(c::BLUE),
        "muted.background": hex(c::CONTROL),
        "muted.foreground": hex(c::INK_MUTED),
        "accent.background": hex(c::CONTROL_HOVER),
        "accent.foreground": hex(c::INK),
        "primary.background": hex(c::BLUE_VIVID),
        "primary.hover.background": hex(c::BLUE_VIVID),
        "primary.active.background": hex(c::BLUE_DEEP),
        "primary.foreground": hex(c::INK_ON_ACCENT),
        "secondary.background": hex(c::CONTROL),
        "secondary.hover.background": hex(c::CONTROL_HOVER),
        "secondary.active.background": hex(c::CONTROL_PRESSED),
        "secondary.foreground": hex(c::INK),
        "popover.background": hex(c::SURFACE_RAISED),
        "popover.foreground": hex(c::INK),
        "overlay": if appearance == Appearance::Dark { "#00000099" } else { "#1a10302e" },
        "selection.background": format!("{}{}", hex(c::BLUE_VIVID), if appearance == Appearance::Dark { "73" } else { "40" }),
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
        "radius": radius::RADIUS_SM as usize,
        "radius.lg": radius::RADIUS_LG as usize,
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
