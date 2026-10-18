//! Saturn's item in the macOS menu bar: a small Saturn, and next to it how many agents are waiting
//! for you across every window, so you see it while Saturn is hidden. Clicking it brings Saturn
//! forward on the next agent that needs you. Settings can turn it off.

use std::cell::RefCell;
use std::collections::HashMap;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSBezierPath, NSColor, NSImage, NSStatusBar, NSStatusItem, NSVariableStatusItemLength};
use objc2_foundation::{NSAffineTransform, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    item: Option<Retained<NSStatusItem>>,
    target: Option<Retained<ClickTarget>>,
    on_click: Option<async_channel::Sender<()>>,
    /// Agents waiting, per window, summed for the item.
    waiting: HashMap<u64, usize>,
}

define_class!(
    // Receives the item's clicks; AppKit needs an Objective-C object as the target.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SaturnMenuBarClickTarget"]
    struct ClickTarget;

    unsafe impl NSObjectProtocol for ClickTarget {}

    impl ClickTarget {
        #[unsafe(method(clicked:))]
        fn clicked(&self, _sender: Option<&AnyObject>) {
            STATE.with_borrow(|state| {
                if let Some(on_click) = &state.on_click {
                    let _ = on_click.try_send(());
                }
            });
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

/// Where clicks on the item go, once, at startup.
pub fn on_click(sender: async_channel::Sender<()>) {
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
                // AppKit keeps the target unretained; `state.target` keeps it alive while the item lives.
                #[expect(unsafe_code, reason = "AppKit's target/action has no safe binding")]
                unsafe {
                    button.setTarget(Some(&target));
                    button.setAction(Some(sel!(clicked:)));
                }
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
    let (title, tip) = match total {
        0 => (String::new(), "Saturn".to_owned()),
        1 => (" 1".to_owned(), "1 agent is waiting for you".to_owned()),
        n => (format!(" {n}"), format!("{n} agents are waiting for you")),
    };
    button.setTitle(&NSString::from_str(&title));
    button.setToolTip(Some(&NSString::from_str(&tip)));
}

/// Saturn's mark at menu bar size: a planet and a tilted ring, as a template image, so macOS
/// draws it black or white to match the menu bar.
fn glyph() -> Retained<NSImage> {
    let draw = RcBlock::new(|_rect: NSRect| -> Bool {
        NSColor::blackColor().set();
        NSBezierPath::bezierPathWithOvalInRect(NSRect::new(NSPoint::new(5.5, 5.5), NSSize::new(7., 7.))).fill();
        let ring =
            NSBezierPath::bezierPathWithOvalInRect(NSRect::new(NSPoint::new(1.25, 6.25), NSSize::new(15.5, 5.5)));
        ring.setLineWidth(1.4);
        let tilt = NSAffineTransform::new();
        tilt.translateXBy_yBy(9., 9.);
        tilt.rotateByDegrees(-20.);
        tilt.translateXBy_yBy(-9., -9.);
        ring.transformUsingAffineTransform(&tilt);
        ring.stroke();
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(18., 18.), false, &draw);
    image.setTemplate(true);
    image
}
