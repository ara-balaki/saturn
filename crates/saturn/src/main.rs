//! Saturn: a terminal grid for running many Claude Code agents side by side.

mod assets;
mod chrome;
mod dock;
mod settings;
mod settings_view;
mod spike;
mod theme;
mod workspace;

use std::borrow::Cow;

use gpui_kit::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, actions, point, px, size,
};

use crate::workspace::{
    CloseTab, Copy, FocusDown, FocusLeft, FocusPane1, FocusPane2, FocusPane3, FocusPane4, FocusPane5, FocusPane6,
    FocusPane7, FocusPane8, FocusPane9, FocusRight, FocusUp, NewTab, NextAttention, NextTab, OpenProject, OpenSettings,
    Paste, Placement, PreviousTab, SplitDown, SplitRight, ToggleEditor, ToggleMaximize, ToggleSidebar, Workspace,
};

actions!(saturn, [Quit]);

/// One static file per weight the design system uses: GPUI draws a variable font only at its default
/// weight. Instanced from the variable fonts with fontTools (OFL 1.1; neither declares a Reserved
/// Font Name).
const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Urbanist-Light.ttf"),
    include_bytes!("../assets/fonts/Urbanist-Regular.ttf"),
    include_bytes!("../assets/fonts/Urbanist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
];

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-t", NewTab, None),
        KeyBinding::new("cmd-d", SplitRight, None),
        KeyBinding::new("cmd-shift-d", SplitDown, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("cmd-shift-]", NextTab, None),
        KeyBinding::new("cmd-shift-[", PreviousTab, None),
        KeyBinding::new("cmd-alt-left", FocusLeft, None),
        KeyBinding::new("cmd-alt-right", FocusRight, None),
        KeyBinding::new("cmd-alt-up", FocusUp, None),
        KeyBinding::new("cmd-alt-down", FocusDown, None),
        KeyBinding::new("cmd-shift-enter", ToggleMaximize, None),
        KeyBinding::new("cmd-shift-a", NextAttention, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-o", OpenProject, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-j", ToggleEditor, None),
        KeyBinding::new("cmd-1", FocusPane1, None),
        KeyBinding::new("cmd-2", FocusPane2, None),
        KeyBinding::new("cmd-3", FocusPane3, None),
        KeyBinding::new("cmd-4", FocusPane4, None),
        KeyBinding::new("cmd-5", FocusPane5, None),
        KeyBinding::new("cmd-6", FocusPane6, None),
        KeyBinding::new("cmd-7", FocusPane7, None),
        KeyBinding::new("cmd-8", FocusPane8, None),
        KeyBinding::new("cmd-9", FocusPane9, None),
    ]);
}

fn set_menus(cx: &mut App) {
    cx.set_menus(vec![
        Menu::new("Saturn").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit Saturn", Quit),
        ]),
        Menu::new("File").items([MenuItem::action("Open Project…", OpenProject)]),
        Menu::new("Terminal").items([
            MenuItem::action("New Tab", NewTab),
            MenuItem::action("Split Right", SplitRight),
            MenuItem::action("Split Down", SplitDown),
            MenuItem::action("Close Tab", CloseTab),
            MenuItem::separator(),
            MenuItem::action("Next Tab", NextTab),
            MenuItem::action("Previous Tab", PreviousTab),
            MenuItem::action("Maximise Pane", ToggleMaximize),
            MenuItem::action("Next Agent Needing You", NextAttention),
        ]),
        Menu::new("View").items([
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::action("Toggle Editor", ToggleEditor),
        ]),
        Menu::new("Edit").items([MenuItem::action("Copy", Copy), MenuItem::action("Paste", Paste)]),
    ]);
}

fn main() {
    let project = settings::startup_project();

    gpui_kit::application().with_assets(assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        theme::apply(settings::load().appearance, cx);
        cx.text_system()
            .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
            .expect("the bundled fonts load");
        bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        set_menus(cx);

        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("Saturn".into()),
                appears_transparent: true,
                // Centred in the 48px title bar.
                traffic_light_position: Some(point(px(18.), px(17.))),
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1600.), px(1000.)), cx))),
            // The glass is the window itself: macOS blurs the desktop behind the translucent ground.
            window_background: WindowBackgroundAppearance::Blurred,
            ..WindowOptions::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| {
                let mut workspace = Workspace::new(project, window, cx);
                match spike::panes() {
                    Some(panes) => {
                        for index in 0..panes {
                            workspace.open_terminal(spike::placement(index, panes), window, cx);
                        }
                    }
                    None => {
                        workspace.open_terminal(Placement::Tab, window, cx);
                    }
                }
                spike::start(&workspace, window, cx);
                workspace
            })
        })
        .expect("the window opens");
        // Automated runs (screenshots, spikes) must not take the keyboard from whatever the user is doing.
        if !spike::is_automated() {
            cx.activate(true);
        }
    });
}
