//! Saturn: a terminal grid for running many Claude Code agents side by side.

mod agents_view;
mod app_view;
mod assets;
mod changes_view;
mod chrome;
mod clickup;
mod dock;
mod editor_view;
mod files_view;
mod github;
mod history_view;
mod impact_view;
mod languages;
mod lsp;
mod menu_bar;
mod notify;
mod palette;
mod problems_view;
mod references_view;
mod schema_view;
mod session;
mod settings;
mod settings_view;
mod spike;
mod status_bar;
mod theme;
mod ticket_picker;
mod tickets_view;
mod updates;
mod usage;
mod workspace;

use std::borrow::Cow;

use gpui_kit::{
    App, AppContext as _, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, actions, point, px, size,
};

use crate::app_view::AppView;
use crate::workspace::{
    CloseTab, CommandPalette, Copy, FocusDown, FocusLeft, FocusPane1, FocusPane2, FocusPane3, FocusPane4, FocusPane5,
    FocusPane6, FocusPane7, FocusPane8, FocusPane9, FocusRight, FocusUp, GoToFile, NewTab, NextAttention, NextTab,
    OpenProject, OpenSettings, OpenTicketPicker, Paste, PreviousTab, SaveFile, SplitDown, SplitRight, TextBigger,
    TextReset, TextSmaller, ToggleEditor, ToggleMaximize, ToggleSidebar, ToggleTickets,
};

actions!(saturn, [Quit, CheckForUpdates]);

/// The login shell's `PATH`, read once at start, for the tools Saturn runs itself.
pub static LOGIN_PATH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// One static file per weight the design system uses: GPUI draws a variable font only at its default
/// weight. Instanced from the variable fonts with fontTools (OFL 1.1; neither declares a Reserved
/// Font Name).
const FONTS: [&[u8]; 6] = [
    include_bytes!("../assets/fonts/Urbanist-Light.ttf"),
    include_bytes!("../assets/fonts/Urbanist-Regular.ttf"),
    include_bytes!("../assets/fonts/Urbanist-Medium.ttf"),
    include_bytes!("../assets/fonts/Urbanist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
];

fn bind_keys(cx: &mut App) {
    saturn_terminal::view::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-n", NewTab, None),
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
        KeyBinding::new("cmd-s", SaveFile, None),
        KeyBinding::new("cmd-shift-n", OpenTicketPicker, None),
        KeyBinding::new("cmd-j", ToggleEditor, None),
        KeyBinding::new("cmd-p", GoToFile, None),
        KeyBinding::new("cmd-alt-b", ToggleTickets, None),
        // ⌘+ is ⌘= without shift on most keyboards; both make text bigger.
        KeyBinding::new("cmd-=", TextBigger, None),
        KeyBinding::new("cmd-+", TextBigger, None),
        KeyBinding::new("cmd--", TextSmaller, None),
        KeyBinding::new("cmd-0", TextReset, None),
        KeyBinding::new("cmd-shift-p", CommandPalette, None),
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
            MenuItem::action("Check for Updates…", CheckForUpdates),
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit Saturn", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Go to File…", GoToFile),
            MenuItem::action("Open Project…", OpenProject),
            MenuItem::action("Save", SaveFile),
        ]),
        Menu::new("Terminal").items([
            MenuItem::action("New Tab", NewTab),
            MenuItem::action("New Agent from Ticket…", OpenTicketPicker),
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
            MenuItem::action("Command Palette…", CommandPalette),
            MenuItem::separator(),
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::action("Toggle Editor", ToggleEditor),
            MenuItem::action("Toggle Tickets", ToggleTickets),
            MenuItem::separator(),
            MenuItem::action("Bigger Text", TextBigger),
            MenuItem::action("Smaller Text", TextSmaller),
            MenuItem::action("Actual Size", TextReset),
        ]),
        Menu::new("Edit").items([MenuItem::action("Copy", Copy), MenuItem::action("Paste", Paste)]),
    ]);
}

fn main() {
    // Off the main thread: an interactive shell's startup files can take a moment.
    std::thread::spawn(|| {
        if let Some(path) = saturn_core::shell_env::login_path() {
            let _ = LOGIN_PATH.set(path);
        }
    });
    let project = settings::startup_project();

    let app = gpui_kit::application().with_assets(assets::Assets);
    // Clicking the Dock icon with no window open (the last one was closed) opens one, as a fresh
    // start does: the welcome screen, where a recent repo comes back with its grid.
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_main_window(settings::startup_project(), cx);
            cx.activate(true);
        }
    });
    app.run(move |cx| {
        gpui_kit::init(cx);
        languages::register();
        theme::apply(settings::load().appearance, cx);
        theme::set_text_size(settings::load().text_size, cx);
        usage::start(cx);
        if settings::load().notifications && !spike::is_automated() {
            notify::request_permission();
        }
        cx.text_system()
            .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
            .expect("the bundled fonts load");
        bind_keys(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &CheckForUpdates, _| updates::check());
        // Only when the window's own handlers didn't get them: nothing had the keyboard.
        cx.on_action(|_: &GoToFile, cx| {
            workspace::in_front(cx, |workspace, window, cx| {
                palette::go_to_file(cx.weak_entity(), workspace.root().to_owned(), window, cx);
            });
        });
        cx.on_action(|_: &CommandPalette, cx| {
            workspace::in_front(cx, |_, window, cx| palette::commands(window, cx));
        });
        set_menus(cx);

        open_main_window(project, cx);
        // Automated runs (screenshots, spikes) must not take the keyboard from whatever the user is doing.
        if !spike::is_automated() {
            cx.activate(true);
            show_menu_bar(cx);
            updates::start();
        } else if std::env::var_os("SATURN_UPDATES_DEBUG").is_some() {
            updates::start();
        }
    });
}

/// Saturn's window: on `project`, or the welcome screen with the recent repos.
fn open_main_window(project: Option<std::path::PathBuf>, cx: &mut App) {
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
    gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| AppView::new(project, window, cx)))
        .expect("the window opens");
}

/// Saturn's menu bar item: its menu brings Saturn forward, on the next agent that needs you when
/// that's what was picked, or quits it.
fn show_menu_bar(cx: &mut App) {
    let (clicks, clicked) = async_channel::unbounded();
    menu_bar::on_click(clicks);
    menu_bar::set_shown(settings::load().menu_bar);
    cx.spawn(async move |cx| {
        while let Ok(choice) = clicked.recv().await {
            cx.update(|cx| {
                if choice == menu_bar::Choice::Quit {
                    cx.quit();
                    return;
                }
                cx.activate(true);
                if cx.windows().is_empty() {
                    open_main_window(settings::startup_project(), cx);
                } else if let Some(window) = cx.windows().into_iter().next() {
                    let _ = window.update(cx, |_, window, cx| {
                        window.activate_window();
                        if choice == menu_bar::Choice::Next {
                            window.dispatch_action(Box::new(NextAttention), cx);
                        }
                    });
                }
            });
        }
    })
    .detach();
}
