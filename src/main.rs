//! Entry point. Wires together: AppKit status item + menu, the caffeinate
//! supervisor, and persistent state.
//!
//! Design notes:
//! - We keep a single `Rc<RefCell<AppState>>` holding the supervisor + current
//!   mode + retained AppKit objects. Menu item actions reach into this state
//!   via an `objc2` `define_class!` controller object that stores the pointer.
//! - Activation policy is set to `.accessory` at runtime so we can run as a
//!   plain binary (no `.app` bundle required) and still avoid a Dock icon.
//! - On every mode change we update the icon tint, the menu check marks, and
//!   the persisted `last_mode`.

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::NSObject;
use objc2::sel;
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem,
    NSVariableStatusItemLength,
};
use objc2_foundation::NSString;

mod icon;
mod state;
mod supervisor;

use state::{load_config, save_config, Config, Mode};
use supervisor::Supervisor;

/// Menu item tag encoding the selected mode (see `tag_for` / `mode_for_tag`).
type TagInt = isize;

fn tag_for(mode: Mode) -> TagInt {
    match mode {
        Mode::Off => 0,
        Mode::IdleOnly => 1,
        Mode::IdleAndDisplay => 2,
    }
}

fn mode_for_tag(tag: TagInt) -> Option<Mode> {
    match tag {
        0 => Some(Mode::Off),
        1 => Some(Mode::IdleOnly),
        2 => Some(Mode::IdleAndDisplay),
        _ => None,
    }
}

/// The mutable application state, shared between menu callbacks.
struct AppState {
    mode: Mode,
    supervisor: Supervisor,
    status_item: Option<Retained<NSStatusItem>>,
    mode_items: Vec<Retained<NSMenuItem>>,
}

impl AppState {
    fn apply_mode(&mut self, mode: Mode, mtm: MainThreadMarker) {
        // Drive the supervisor first. On failure we keep the UI consistent by
        // falling back to Off visually.
        match self.supervisor.enter(mode) {
            Ok(_) => self.mode = mode,
            Err(e) => {
                eprintln!("cafe: {e}");
                // Stay visually Off; reflect that we could not arm.
                self.mode = Mode::Off;
            }
        }

        // Update the icon + tooltip on the status button.
        if let Some(item) = &self.status_item {
            if let Some(button) = item.button(mtm) {
                // Re-bake the icon with the new mode color and swap it in.
                if let Some(img) = icon::image_for_mode(self.mode) {
                    button.setImage(Some(&img));
                }
                button.setToolTip(Some(&NSString::from_str(self.mode.tooltip())));
            }
        }

        // Update check marks on the three mode items.
        for (i, m) in Mode::ALL.iter().enumerate() {
            if let Some(item) = self.mode_items.get(i) {
                let on = *m == self.mode;
                item.setState(if on { 1 } else { 0 });
            }
        }

        // Persist last mode (non-fatal).
        save_config(&Config {
            last_mode: self.mode,
        })
        .ok();
    }
}

/// The ObjC controller class that owns a pointer to `Rc<RefCell<AppState>>` and
/// receives menu item actions.
#[derive(Default)]
struct CafeControllerIvars {
    /// Set once, after the status item + menu are wired up. Held in a RefCell
    /// because the ivars themselves are only accessible through a shared
    /// reference after construction.
    state: RefCell<Option<Rc<RefCell<AppState>>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "CafeController"]
    #[ivars = CafeControllerIvars]
    struct CafeController;

    impl CafeController {
        /// Action target for the three mode menu items. The sender's tag tells
        /// us which mode was chosen.
        #[unsafe(method(selectMode:))]
        fn select_mode(&self, sender: *mut NSObject) {
            // Menu actions always run on the main thread.
            let mtm = MainThreadMarker::new().expect("menu action on main thread");
            // Read the tag from the sending menu item.
            // SAFETY: `sender` is the menu item that fired this action.
            let tag: TagInt = unsafe { msg_send![sender, tag] };
            let Some(mode) = mode_for_tag(tag) else { return };
            let state = self.ivars().state.borrow().clone();
            let Some(state) = state else {
                return;
            };
            state.borrow_mut().apply_mode(mode, mtm);
        }

        /// Action target for the Quit item.
        #[unsafe(method(quit:))]
        fn quit(&self, _sender: *mut NSObject) {
            // Stop any caffeinate child before terminating so we never leave a
            // dangling process even if NSApp terminates abruptly.
            let state = self.ivars().state.borrow().clone();
            if let Some(state) = state {
                let _ = state.borrow_mut().supervisor.enter(Mode::Off);
            }
            let mtm = MainThreadMarker::new().expect("menu action on main thread");
            let app = NSApplication::sharedApplication(mtm);
            app.terminate(None);
        }
    }
);

impl CafeController {
    /// Construct a controller with default (empty) ivars. The shared app state
    /// is injected afterwards via the `RefCell` ivar.
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(CafeControllerIvars::default());
        // SAFETY: `NSObject`'s `init` is inherited; `this` has +1 retain count
        // and its ivars were initialized above.
        unsafe { msg_send![super(this), init] }
    }
}

fn build_menu(controller: &Retained<CafeController>, mtm: MainThreadMarker) -> Retained<NSMenu> {
    use objc2::runtime::AnyObject;

    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);

    // Header line.
    let header = NSMenuItem::new(mtm);
    header.setTitle(&NSString::from_str("☕ cafe"));
    header.setEnabled(false);
    menu.addItem(&header);

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Mode items.
    let controller_ref: &AnyObject = controller;
    for mode in Mode::ALL {
        let item = NSMenuItem::new(mtm);
        item.setTitle(&NSString::from_str(mode.label()));
        item.setTag(tag_for(mode));
        // SAFETY: setting a target/action on a menu item is standard AppKit.
        unsafe {
            item.setTarget(Some(controller_ref));
            item.setAction(Some(sel!(selectMode:)));
        }
        item.setEnabled(true);
        item.setState(if mode == Mode::Off { 1 } else { 0 });
        menu.addItem(&item);
    }

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Quit.
    let quit = NSMenuItem::new(mtm);
    quit.setTitle(&NSString::from_str("Quit cafe"));
    // SAFETY: as above.
    unsafe {
        quit.setTarget(Some(controller_ref));
        quit.setAction(Some(sel!(quit:)));
    }
    menu.addItem(&quit);

    menu
}

fn main() {
    let mtm = MainThreadMarker::new().expect("cafe must run on the main thread");

    let app = NSApplication::sharedApplication(mtm);
    // Run as an accessory (menu bar only, no Dock icon).
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // Load persisted state. We always start in Off for safety; last_mode is
    // remembered only for potential future "restore" UX and is not applied.
    let _config = load_config();

    // Status bar item.
    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

    // Set the icon (color baked in per mode) + tooltip.
    {
        if let Some(image) = icon::image_for_mode(Mode::Off) {
            if let Some(button) = status_item.button(mtm) {
                button.setImage(Some(&image));
                button.setToolTip(Some(&NSString::from_str(Mode::Off.tooltip())));
            }
        }
    }

    // Controller.
    let controller = CafeController::new();

    // Menu.
    let menu = build_menu(&controller, mtm);

    // Collect mode item references after building. The header is index 0, the
    // first separator is index 1, so mode items start at index 2.
    let mode_items: Vec<Retained<NSMenuItem>> = Mode::ALL
        .iter()
        .enumerate()
        .map(|(i, _)| {
            menu.itemAtIndex((2 + i) as isize)
                .expect("mode item present")
        })
        .collect();

    // Wire status item's menu.
    status_item.setMenu(Some(&menu));

    // Initialize app state.
    let state = Rc::new(RefCell::new(AppState {
        mode: Mode::Off,
        supervisor: Supervisor::new(),
        status_item: Some(status_item),
        mode_items,
    }));

    // Hand the state to the controller via the RefCell ivar.
    *controller.ivars().state.borrow_mut() = Some(state.clone());

    // Run the event loop. This blocks until the app terminates.
    autoreleasepool(|_| {
        app.run();
    });
}
