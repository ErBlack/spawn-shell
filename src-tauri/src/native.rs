use std::ffi::c_void;

use objc2::runtime::AnyObject;
use objc2::{ClassType, MainThreadMarker, define_class};
use objc2_app_kit::{
    NSApplication, NSEvent, NSPanel, NSScreen, NSStatusWindowLevel, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

define_class!(
    #[unsafe(super(NSPanel))]
    #[name = "SummonPanel"]
    struct SummonPanel;

    impl SummonPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

pub fn make_panel(_mtm: MainThreadMarker, ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    unsafe {
        let _ = AnyObject::set_class(window, SummonPanel::class());
    }
    window.setStyleMask(NSWindowStyleMask::NonactivatingPanel);
    window.setLevel(NSStatusWindowLevel);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
}

fn mouse_in_rect(point: NSPoint, rect: NSRect) -> bool {
    point.x >= rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y > rect.origin.y
        && point.y <= rect.origin.y + rect.size.height
}

pub fn position_at_cursor(mtm: MainThreadMarker, ns_window: *mut c_void, height_percent: u32) {
    if ns_window.is_null() {
        return;
    }
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };

    let mouse = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    let frame = screens
        .iter()
        .find_map(|screen| {
            let frame = screen.frame();
            mouse_in_rect(mouse, frame).then_some(frame)
        })
        .or_else(|| NSScreen::mainScreen(mtm).map(|screen| screen.frame()));

    let Some(frame) = frame else {
        return;
    };

    let height = frame.size.height * f64::from(height_percent) / 100.0;
    let origin = NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height - height);
    let size = NSSize::new(frame.size.width, height);
    window.setFrame_display(NSRect::new(origin, size), true);
}

pub fn show(mtm: MainThreadMarker, ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    NSApplication::sharedApplication(mtm).activate();
    if let Some(content) = window.contentView() {
        window.makeFirstResponder(Some(&**content));
    }
    window.orderFrontRegardless();
    window.makeKeyWindow();
}

pub fn order_out(_mtm: MainThreadMarker, ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let window: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
    window.orderOut(None);
}
