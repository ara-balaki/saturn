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
// Glass: how much of the blurred desktop behind the window each layer covers. They stack (ground,
// then a pane, then its well), so the gaps show the most desktop and text sits on the most cover:
// about 30% in the gaps, 60% on a pane's strip, 75% behind terminal and editor text.
pub const GLASS_GROUND: f32 = 0.3;
pub const GLASS_PANEL: f32 = 0.43;
pub const GLASS_WELL: f32 = 0.38;

/// fonts and radii instead of the library's grey defaults, for one appearance.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "radii are small whole pixels")]
fn component_theme(appearance: Appearance) -> gpui_kit::component::ThemeConfig {
    use saturn_design::{color as c, font, radius};

    let hex = |token: Token| hex(token, appearance);
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
        // Light, because the window under a dialog is frosted too (chrome::frost).
        "overlay": if appearance == Appearance::Dark { "#00000059" } else { "#1a10301f" },
        "selection.background": format!("{}{}", hex(c::BLUE_VIVID), if appearance == Appearance::Dark { "73" } else { "40" }),
        "caret": hex(c::INK),
        "switch.background": hex(c::CONTROL_PRESSED),
        "switch.thumb.background": hex(c::INK_ON_ACCENT),
        "scrollbar.background": "#00000000",
        "scrollbar.thumb.background": hex(c::LINE_STRONG),
        "scrollbar.thumb.hover.background": hex(c::INK_FAINT),
        // The diff view's added and removed lines.
        "success.background": hex(c::TERM_GREEN),
        "success.foreground": hex(c::INK_ON_SIGNAL),
        "danger.background": hex(c::RED),
        "danger.foreground": hex(c::INK_ON_ACCENT),
    });
    let highlight = serde_json::json!({
        // The editor sits in a well, which draws the glass; its own background stays clear.
        "editor.background": "#00000000",
        "editor.foreground": hex(c::INK_BODY),
        "editor.active_line.background": hex(c::SURFACE_RAISED),
        "editor.line_number": hex(c::INK_FAINT),
        "editor.active_line_number": hex(c::INK_MUTED),
        "editor.invisible": hex(c::LINE_STRONG),
        "editor.gutter.background": "#00000000",
        "syntax": syntax(appearance),
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
        "highlight": highlight,
    }))
    .expect("Saturn's theme config is valid")
}

/// `#rrggbb` for `appearance`, as the theme config takes colours as strings.
fn hex(token: Token, appearance: Appearance) -> String {
    let color = match appearance {
        Appearance::Dark => token.dark,
        Appearance::Light => token.light,
    };
    format!("#{:06x}", color.rgb)
}

/// The editor's colours: the terminal's palette, used the way most editors use it (One Dark's
/// roles), so code reads the same here as in the panes next to it. Keys and headings get colour
/// too, since in YAML, JSON and Markdown that is most of the file. Bold only where the text
/// itself is a heading or strong; JetBrains Mono ships no italic here.
fn syntax(appearance: Appearance) -> serde_json::Value {
    use saturn_design::color as c;

    let style = |token: Token| serde_json::json!({ "color": hex(token, appearance) });
    let bold = |token: Token| serde_json::json!({ "color": hex(token, appearance), "font_weight": 700 });
    serde_json::json!({
        "keyword": style(c::TERM_MAGENTA),
        "preproc": style(c::TERM_MAGENTA),
        "function": style(c::TERM_BLUE),
        "constructor": style(c::TERM_BLUE),
        "type": style(c::TERM_CYAN),
        "enum": style(c::TERM_CYAN),
        "variant": style(c::TERM_CYAN),
        "label": style(c::TERM_CYAN),
        "property": style(c::TERM_RED),
        "tag": style(c::TERM_RED),
        "variable": style(c::INK_BODY),
        "variable.special": style(c::TERM_RED),
        "attribute": style(c::AMBER_TEXT),
        "string": style(c::TERM_GREEN),
        "string.special": style(c::TERM_RED),
        "string.special.symbol": style(c::TERM_CYAN),
        "string.escape": style(c::TERM_CYAN),
        "string.regex": style(c::TERM_CYAN),
        "number": style(c::AMBER_TEXT),
        "boolean": style(c::AMBER_TEXT),
        "constant": style(c::AMBER_TEXT),
        "comment": style(c::INK_FAINT),
        "comment_doc": style(c::INK_FAINT),
        "operator": style(c::TERM_CYAN),
        "punctuation": style(c::INK_MUTED),
        "punctuation.bracket": style(c::INK_MUTED),
        "punctuation.delimiter": style(c::INK_MUTED),
        "punctuation.special": style(c::INK_FAINT),
        "punctuation.list_marker": style(c::AMBER_TEXT),
        "embedded": style(c::INK_BODY),
        // Markdown.
        "title": bold(c::TERM_BLUE),
        "emphasis": style(c::TERM_MAGENTA),
        "emphasis.strong": bold(c::INK),
        "link_text": style(c::TERM_BLUE),
        "link_uri": style(c::TERM_CYAN),
        "text.literal": style(c::TERM_GREEN),
        "text.code.span": style(c::TERM_GREEN),
    })
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
