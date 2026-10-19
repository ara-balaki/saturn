//! Saturn's item in the macOS menu bar: a small Saturn, and next to it how many agents are waiting
//! for you across every window, so you see it while Saturn is hidden. Clicking it opens a menu:
//! how many need you, a way to the next one, Saturn itself, and Quit. Settings can turn it off.

use std::cell::RefCell;
use std::collections::HashMap;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSAffineTransform, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// What was picked in the item's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Go to the next agent that needs you.
    Next,
    /// Bring Saturn forward, opening a window if none is open.
    Show,
    Quit,
}

#[derive(Default)]
struct State {
    item: Option<Retained<NSStatusItem>>,
    target: Option<Retained<ClickTarget>>,
    on_click: Option<async_channel::Sender<Choice>>,
    /// Agents waiting, per window, summed for the item.
    waiting: HashMap<u64, usize>,
}

define_class!(
    // Receives the menu's choices; AppKit needs an Objective-C object as the target.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SaturnMenuBarClickTarget"]
    struct ClickTarget;

    unsafe impl NSObjectProtocol for ClickTarget {}

    impl ClickTarget {
        #[unsafe(method(next:))]
        fn next(&self, _sender: Option<&AnyObject>) {
            choose(Choice::Next);
        }

        #[unsafe(method(show:))]
        fn show(&self, _sender: Option<&AnyObject>) {
            choose(Choice::Show);
        }

        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            choose(Choice::Quit);
        }
    }
);

impl ClickTarget {
    #[expect(unsafe_code, reason = "calling NSObject's init on a freshly allocated object is what objc2 asks for")]
    fn new(main_thread: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(main_thread).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

fn choose(choice: Choice) {
    STATE.with_borrow(|state| {
        if let Some(on_click) = &state.on_click {
            let _ = on_click.try_send(choice);
        }
    });
}

/// Where the menu's choices go, once, at startup.
pub fn on_click(sender: async_channel::Sender<Choice>) {
    STATE.with_borrow_mut(|state| state.on_click = Some(sender));
}

/// Shows the item, or takes it away.
pub fn set_shown(shown: bool) {
    let Some(main_thread) = MainThreadMarker::new() else { return };
    STATE.with_borrow_mut(|state| {
        if shown && state.item.is_none() {
            let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
            let target = ClickTarget::new(main_thread);
            if let Some(button) = item.button(main_thread) {
                button.setImage(Some(&glyph()));
            }
            state.item = Some(item);
            state.target = Some(target);
            show_count(state, main_thread);
        } else if !shown && let Some(item) = state.item.take() {
            NSStatusBar::systemStatusBar().removeStatusItem(&item);
            state.target = None;
        }
    });
}

/// How many agents in window `window` are waiting for you.
pub fn set_waiting(window: u64, count: usize) {
    let Some(main_thread) = MainThreadMarker::new() else { return };
    STATE.with_borrow_mut(|state| {
        if count == 0 {
            state.waiting.remove(&window);
        } else {
            state.waiting.insert(window, count);
        }
        show_count(state, main_thread);
    });
}

fn show_count(state: &State, main_thread: MainThreadMarker) {
    let Some(button) = state.item.as_ref().and_then(|item| item.button(main_thread)) else { return };
    let total: usize = state.waiting.values().sum();
    let (title, summary) = match total {
        0 => (String::new(), "No agent needs you".to_owned()),
        1 => (" 1".to_owned(), "1 agent needs you".to_owned()),
        n => (format!(" {n}"), format!("{n} agents need you")),
    };
    button.setTitle(&NSString::from_str(&title));
    button.setToolTip(Some(&NSString::from_str(&format!("Saturn: {summary}"))));
    if let (Some(item), Some(target)) = (&state.item, &state.target) {
        item.setMenu(Some(&menu(&summary, total > 0, target, main_thread)));
    }
}

/// The item's menu: the count, then what you can do from it.
fn menu(summary: &str, waiting: bool, target: &ClickTarget, main_thread: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::new(main_thread);
    // Enabled or not as set here, not by whether the target answers.
    menu.setAutoenablesItems(false);
    let entry = |title: &str, action: Option<objc2::runtime::Sel>, enabled: bool| {
        #[expect(unsafe_code, reason = "AppKit's menu items and target/action have no safe binding")]
        let item = unsafe {
            let item = NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(main_thread),
                &NSString::from_str(title),
                action,
                &NSString::from_str(""),
            );
            // AppKit keeps the target unretained; `State::target` keeps it alive while the item lives.
            item.setTarget(Some(target));
            item
        };
        item.setEnabled(enabled);
        item
    };
    menu.addItem(&entry(summary, None, false));
    menu.addItem(&entry("Go to the next one", Some(sel!(next:)), waiting));
    menu.addItem(&NSMenuItem::separatorItem(main_thread));
    menu.addItem(&entry("Show Saturn", Some(sel!(show:)), true));
    menu.addItem(&entry("Quit Saturn", Some(sel!(quit:)), true));
    menu
}

/// Saturn's mark at menu bar size, as the app icon draws it: a tilted ring rising to the right,
/// its far half behind the planet and its near half in front. A template image, so macOS draws
/// it black or white to match the menu bar.
fn glyph() -> Retained<NSImage> {
    let draw = RcBlock::new(|_rect: NSRect| -> Bool {
        NSColor::blackColor().set();
        // The ring in halves around (9, 9): the far half above the centre line, the near below.
        let (centre, across, deep) = (9.0_f64, 7.75_f64, 2.6_f64);
        let half = |sign: f64| {
            const K: f64 = 0.552_284_75;
            let path = NSBezierPath::bezierPath();
            path.moveToPoint(NSPoint::new(centre + across, centre));
            path.curveToPoint_controlPoint1_controlPoint2(
                NSPoint::new(centre, centre + sign * deep),
                NSPoint::new(centre + across, centre + sign * K * deep),
                NSPoint::new(centre + K * across, centre + sign * deep),
            );
            path.curveToPoint_controlPoint1_controlPoint2(
                NSPoint::new(centre - across, centre),
                NSPoint::new(centre - K * across, centre + sign * deep),
                NSPoint::new(centre - across, centre + sign * K * deep),
            );
            path.setLineWidth(1.3);
            path.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
            // Rising to the right, as on the icon (AppKit's y points up, so counter-clockwise).
            let tilt = NSAffineTransform::new();
            tilt.translateXBy_yBy(centre, centre);
            tilt.rotateByDegrees(20.);
            tilt.translateXBy_yBy(-centre, -centre);
            path.transformUsingAffineTransform(&tilt);
            path
        };
        half(1.).stroke();
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(NSPoint::new(5., 5.), NSSize::new(8., 8.))).fill();
        // One colour can't put a ring in front of a planet, so the near half cuts a thin gap into
        // the planet first, then is drawn in it.
        let near = half(-1.);
        if let Some(context) = objc2_app_kit::NSGraphicsContext::currentContext() {
            context.setCompositingOperation(objc2_app_kit::NSCompositingOperation::Clear);
            near.setLineWidth(3.);
            near.stroke();
            context.setCompositingOperation(objc2_app_kit::NSCompositingOperation::SourceOver);
            near.setLineWidth(1.3);
            // Where the halves meet at the ring's ends, the gap bit into the far half: mend it.
            half(1.).stroke();
        }
        near.stroke();
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(18., 18.), false, &draw);
    image.setTemplate(true);
    image
}
