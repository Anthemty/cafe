//! Entry point. Wires together: AppKit status item + menu, the caffeinate
//! supervisor, and persistent state.
//!
//! Design notes:
//! - `Rc<RefCell<AppState>>` holds the supervisor, current mode, retained
//!   AppKit objects, and caches. The `define_class!` controller is both the
//!   menu target/action receiver AND the `NSMenuDelegate` (it syncs child
//!   liveness + agent status every time the menu opens) AND the `NSTimer`
//!   target (it ticks countdowns / agent polling).
//! - Activation policy is `.accessory` at runtime so the app runs as a plain
//!   binary (no `.app` bundle required) with no Dock icon.
//! - Mode changes update: caffeinate child, icon, check marks, menu labels
//!   (countdown), the persisted config, and (in auto mode) nothing else — auto
//!   re-arms via its own poll loop.

use std::cell::RefCell;
use std::rc::Rc;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::GlobalHotKeyManager;
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::sel;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSControlStateValueOff, NSControlStateValueOn,
    NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSDate, NSString, NSTimeInterval, NSTimer};

mod icon;
mod state;
mod supervisor;

use state::{
    agents_running, countdown_text, load_config, save_config, set_login_item, timer_label, Config,
    Lang, Mode, TIMER_PRESETS,
};
use supervisor::Supervisor;

/// Menu item tag encoding (see `tag_for` / `mode_for_tag`).
type TagInt = isize;

/// Tag namespaces so the selector knows what kind of item fired.
const TAG_MODE_BASE: TagInt = 100; // 100..=102 → modes
const TAG_TIMER_BASE: TagInt = 200; // 200+i → TIMER_PRESETS[i]
const TAG_LOGIN_ITEM: TagInt = 300;
const TAG_AUTO_WATCH: TagInt = 301;
const TAG_LANG_ITEM: TagInt = 302;

fn tag_for(mode: Mode) -> TagInt {
    TAG_MODE_BASE
        + match mode {
            Mode::Off => 0,
            Mode::IdleOnly => 1,
            Mode::IdleAndDisplay => 2,
        }
}

fn mode_for_tag(tag: TagInt) -> Option<Mode> {
    match tag {
        100 => Some(Mode::Off),
        101 => Some(Mode::IdleOnly),
        102 => Some(Mode::IdleAndDisplay),
        _ => None,
    }
}

/// The mutable application state, shared between menu/timer callbacks.
struct AppState {
    mode: Mode,
    supervisor: Supervisor,
    status_item: Option<Retained<NSStatusItem>>,
    /// One menu item per entry in `Mode::ALL`, same order.
    mode_items: Vec<Retained<NSMenuItem>>,
    /// Timer preset items (same order as `TIMER_PRESETS`).
    timer_items: Vec<Retained<NSMenuItem>>,
    /// The "Launch at Login" item (check state mirrors the plist).
    login_item: Option<Retained<NSMenuItem>>,
    /// The "Auto: watch agents" item.
    auto_item: Option<Retained<NSMenuItem>>,
    /// The language toggle item (title always shows the target language).
    lang_item: Option<Retained<NSMenuItem>>,
    /// The Quit item (localized title).
    quit_item: Option<Retained<NSMenuItem>>,
    /// Last caffeinate spawn error, surfaced in the menu when set.
    last_spawn_error: Option<String>,
    /// Countdown deadline for timed sessions (Interval since 2001-01-01).
    deadline: Option<NSTimeInterval>,
    /// Countdown refresh timer.
    countdown_timer: Option<Retained<NSTimer>>,
    /// Agent-watch poll timer + last observed agent state.
    agent_timer: Option<Retained<NSTimer>>,
    agents_were_running: bool,
    /// Whether auto-watch is armed (config-persisted).
    auto_watch: bool,
    /// UI language (config-persisted).
    lang: Lang,
    /// Icon cache (renders each mode icon once).
    icons: icon::IconCache,
    /// Config as last loaded/saved.
    config: Config,
}

impl AppState {
    fn new() -> Self {
        let config = load_config();
        Self {
            mode: Mode::Off,
            supervisor: Supervisor::new(),
            status_item: None,
            mode_items: Vec::new(),
            timer_items: Vec::new(),
            login_item: None,
            auto_item: None,
            lang_item: None,
            quit_item: None,
            last_spawn_error: None,
            deadline: None,
            countdown_timer: None,
            agent_timer: None,
            agents_were_running: false,
            auto_watch: config.auto_watch,
            lang: config.lang,
            icons: icon::IconCache::new(),
            config,
        }
    }

    /// Persist the config's mutable bits from live state.
    fn persist(&mut self) {
        self.config.last_mode = self.mode;
        self.config.auto_watch = self.auto_watch;
        self.config.lang = self.lang;
        save_config(&self.config).ok();
    }

    /// Apply `mode` (manual selection): drive supervisor, update all UI,
    /// clear any timed session.
    fn apply_mode(&mut self, mode: Mode, mtm: MainThreadMarker) {
        // Manual selection cancels auto-watch and any countdown.
        self.auto_watch = false;
        self.deadline = None;

        match self.supervisor.enter(mode, None) {
            Ok(_) => {
                self.mode = mode;
                self.last_spawn_error = None;
            }
            Err(e) => {
                eprintln!("cafe: {e}");
                self.last_spawn_error = Some(e.to_string());
                self.mode = Mode::Off;
            }
        }
        self.refresh_ui(mtm);
        self.persist();
    }

    /// Arm a timed session: keep awake with `mode`-equivalent flags for
    /// `minutes`, then auto-revert to Off.
    fn apply_timer(&mut self, minutes: u64, mtm: MainThreadMarker) {
        let secs = minutes * 60;
        match self.supervisor.enter(Mode::IdleAndDisplay, Some(secs)) {
            Ok(_) => {
                self.mode = Mode::IdleAndDisplay;
                self.last_spawn_error = None;
                self.deadline = Some(NSDate::now().timeIntervalSinceReferenceDate() + secs as f64);
            }
            Err(e) => {
                eprintln!("cafe: {e}");
                self.last_spawn_error = Some(e.to_string());
                self.mode = Mode::Off;
                self.deadline = None;
            }
        }
        self.refresh_ui(mtm);
        self.persist();
    }

    /// Auto mode decision: armed iff agents are running.
    fn auto_decide(&mut self, mtm: MainThreadMarker) {
        let running = agents_running();
        let changed = running != self.agents_were_running;
        self.agents_were_running = running;
        if !self.auto_watch {
            return;
        }
        if !changed {
            // Still refresh the countdown label if a timed session is active.
            self.refresh_ui(mtm);
            return;
        }
        let target = if running {
            Mode::IdleAndDisplay
        } else {
            Mode::Off
        };
        match self.supervisor.enter(target, None) {
            Ok(_) => {
                self.mode = target;
                self.last_spawn_error = None;
            }
            Err(e) => {
                eprintln!("cafe: {e}");
                self.last_spawn_error = Some(e.to_string());
                self.mode = Mode::Off;
            }
        }
        self.refresh_ui(mtm);
        // Auto mode intentionally does not persist last_mode churn.
    }

    /// Toggle launch-at-login from the menu.
    fn toggle_login_item(&mut self, mtm: MainThreadMarker) {
        let want = !state::login_item_enabled();
        match set_login_item(want) {
            Ok(()) => self.refresh_ui(mtm),
            Err(e) => {
                eprintln!("cafe: {e}");
                self.last_spawn_error = Some(e);
            }
        }
    }

    /// Toggle auto-watch; when enabled, decide immediately.
    fn toggle_auto_watch(&mut self, mtm: MainThreadMarker) {
        self.auto_watch = !self.auto_watch;
        if self.auto_watch {
            self.agents_were_running = agents_running();
            let target = if self.agents_were_running {
                Mode::IdleAndDisplay
            } else {
                Mode::Off
            };
            let _ = self.supervisor.enter(target, None);
            self.mode = target;
        }
        self.refresh_ui(mtm);
        self.persist();
    }

    /// Sync every UI surface with current state: icon, tooltip, check marks,
    /// localized titles, error line.
    fn refresh_ui(&mut self, mtm: MainThreadMarker) {
        let lang = self.lang;

        // Icon + tooltip.
        if let Some(item) = &self.status_item {
            if let Some(button) = item.button(mtm) {
                if let Some(img) = self.icons.get(self.mode) {
                    button.setImage(Some(&img));
                }
                let tip = self.status_tooltip();
                button.setToolTip(Some(&NSString::from_str(&tip)));
            }
        }

        // Localized titles for the dynamic items.
        for (i, m) in Mode::ALL.iter().enumerate() {
            if let Some(item) = self.mode_items.get(i) {
                item.setTitle(&NSString::from_str(m.label(lang)));
                item.setState(if *m == self.mode {
                    NSControlStateValueOn
                } else {
                    NSControlStateValueOff
                });
            }
        }
        for (i, item) in self.timer_items.iter().enumerate() {
            item.setTitle(&NSString::from_str(&timer_label(TIMER_PRESETS[i], lang)));
        }
        if let Some(item) = &self.login_item {
            let title = match lang {
                Lang::En => "Launch at Login",
                Lang::Zh => "登录时启动",
            };
            item.setTitle(&NSString::from_str(title));
            item.setState(if state::login_item_enabled() {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        if let Some(item) = &self.auto_item {
            let title = match lang {
                Lang::En => "Auto: Watch Agents",
                Lang::Zh => "自动:监测 Agent",
            };
            item.setTitle(&NSString::from_str(title));
            item.setState(if self.auto_watch {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        if let Some(item) = &self.lang_item {
            item.setTitle(&NSString::from_str(lang.toggle_label()));
        }
        if let Some(item) = &self.quit_item {
            let title = match lang {
                Lang::En => "Quit cafe",
                Lang::Zh => "退出 cafe",
            };
            item.setTitle(&NSString::from_str(title));
        }

        // Timer items: checked only while a matching countdown is live.
        for (i, item) in self.timer_items.iter().enumerate() {
            let on = self.deadline.is_some_and(|d| {
                let mins = TIMER_PRESETS[i];
                (d - NSDate::now().timeIntervalSinceReferenceDate()) as u64 / 60 == mins
            });
            item.setState(if on {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
    }

    /// Switch UI language (menu toggle), then rebuild all localized strings.
    fn apply_lang(&mut self, mtm: MainThreadMarker) {
        self.lang = self.lang.next();
        self.refresh_ui(mtm);
        self.persist();
    }

    /// Status tooltip with countdown, if any.
    fn status_tooltip(&self) -> String {
        let base = self.mode.tooltip(self.lang);
        if let Some(d) = self.deadline {
            let now = NSDate::now().timeIntervalSinceReferenceDate();
            let remain = ((d - now).max(0.0) / 60.0).ceil() as u64;
            return format!("{base} — {}", countdown_text(remain, self.lang));
        }
        base.to_string()
    }

    /// Tick: called by the countdown timer every second. When the deadline
    /// passes, disarm and refresh.
    fn tick_countdown(&mut self, mtm: MainThreadMarker) {
        if let Some(d) = self.deadline {
            if NSDate::now().timeIntervalSinceReferenceDate() >= d {
                self.deadline = None;
                let _ = self.supervisor.enter(Mode::Off, None);
                self.mode = Mode::Off;
                self.refresh_ui(mtm);
            } else {
                // Update just the tooltip (cheap).
                if let Some(item) = &self.status_item {
                    if let Some(button) = item.button(mtm) {
                        button.setToolTip(Some(&NSString::from_str(&self.status_tooltip())));
                    }
                }
            }
        }
    }
}

/// Set by the (background-thread) global-hotkey handler; consumed on the next
/// main-thread tick. A static because the hotkey closure must be Send + Sync
/// while the app state is main-thread-only.
static HOTKEY_REQUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The ObjC controller class: menu target/action receiver, NSMenuDelegate,
/// and NSTimer target in one.
#[derive(Default)]
struct CafeControllerIvars {
    state: RefCell<Option<Rc<RefCell<AppState>>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "CafeController"]
    #[thread_kind = MainThreadOnly]
    #[ivars = CafeControllerIvars]
    struct CafeController;

    unsafe impl NSObjectProtocol for CafeController {}

    impl CafeController {
        /// Unified action: dispatch on the sender's tag namespace.
        #[unsafe(method(selectAction:))]
        fn select_action(&self, sender: *mut NSObject) {
            let mtm = MainThreadMarker::new().expect("menu action on main thread");
            // SAFETY: `sender` is the menu item that fired this action.
            let tag: TagInt = unsafe { msg_send![sender, tag] };
            let state = self.ivars().state.borrow().clone();
            let Some(state) = state else { return };

            if let Some(mode) = mode_for_tag(tag) {
                state.borrow_mut().apply_mode(mode, mtm);
            } else if tag >= TAG_TIMER_BASE && tag < TAG_TIMER_BASE + TIMER_PRESETS.len() as TagInt
            {
                let i = (tag - TAG_TIMER_BASE) as usize;
                state.borrow_mut().apply_timer(TIMER_PRESETS[i], mtm);
            } else if tag == TAG_LOGIN_ITEM {
                state.borrow_mut().toggle_login_item(mtm);
            } else if tag == TAG_AUTO_WATCH {
                state.borrow_mut().toggle_auto_watch(mtm);
            } else if tag == TAG_LANG_ITEM {
                state.borrow_mut().apply_lang(mtm);
            }
        }

        /// Quit: disarm the child first so we never leave a dangling process.
        #[unsafe(method(quitAction:))]
        fn quit_action(&self, _sender: *mut NSObject) {
            let state = self.ivars().state.borrow().clone();
            if let Some(state) = state {
                let _ = state.borrow_mut().supervisor.enter(Mode::Off, None);
            }
            let mtm = MainThreadMarker::new().expect("menu action on main thread");
            let app = NSApplication::sharedApplication(mtm);
            app.terminate(None);
        }

        /// NSTimer target for countdown ticks (1s). Also consumes hotkey
        /// requests flagged from the background event thread.
        #[unsafe(method(tickFire:))]
        fn tick_fire(&self, _sender: *mut NSObject) {
            let mtm = MainThreadMarker::new().expect("timer on main thread");
            let state = self.ivars().state.borrow().clone();
            let Some(state) = state else { return };

            if HOTKEY_REQUEST.swap(false, std::sync::atomic::Ordering::Relaxed) {
                let next = state.borrow().mode.next_in_cycle();
                state.borrow_mut().apply_mode(next, mtm);
                return;
            }
            state.borrow_mut().tick_countdown(mtm);
        }

        /// NSTimer target for the agent-watch poll (5s).
        #[unsafe(method(agentFire:))]
        fn agent_fire(&self, _sender: *mut NSObject) {
            let mtm = MainThreadMarker::new().expect("timer on main thread");
            let state = self.ivars().state.borrow().clone();
            let Some(state) = state else { return };
            state.borrow_mut().auto_decide(mtm);
        }
    }
);

// NSMenuDelegate implementation: sync state every time the menu opens.
unsafe impl NSMenuDelegate for CafeController {
    fn menuNeedsUpdate(&self, _menu: &NSMenu) {
        let mtm = MainThreadMarker::new().expect("menu delegate on main thread");
        let state = self.ivars().state.borrow().clone();
        let Some(state) = state else { return };

        {
            let mut s = state.borrow_mut();
            // Reap externally-killed / timed-out children so the UI never lies.
            if s.supervisor.reap_if_exited() && s.deadline.is_none() {
                s.mode = Mode::Off;
            }
            s.refresh_ui(mtm);
        }
    }
}

impl CafeController {
    /// Construct a controller with default (empty) ivars; state is injected
    /// afterwards via the `RefCell` ivar.
    fn new() -> Retained<Self> {
        let mtm = MainThreadMarker::new().expect("controller on main thread");
        let this = Self::alloc(mtm).set_ivars(CafeControllerIvars::default());
        // SAFETY: `NSObject`'s `init` is inherited; `this` has +1 retain count
        // and its ivars were initialized above.
        unsafe { msg_send![super(this), init] }
    }
}

/// A menu item bound to `selectAction:` with the given tag.
fn action_item(title: &str, tag: TagInt, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = NSMenuItem::new(mtm);
    item.setTitle(&NSString::from_str(title));
    item.setTag(tag);
    item
}

/// Menu build output: the menu plus handles to items the app must mutate later.
struct MenuParts {
    menu: Retained<NSMenu>,
    mode_items: Vec<Retained<NSMenuItem>>,
    timer_items: Vec<Retained<NSMenuItem>>,
    login_item: Retained<NSMenuItem>,
    auto_item: Retained<NSMenuItem>,
    lang_item: Retained<NSMenuItem>,
    quit_item: Retained<NSMenuItem>,
}

fn build_menu(
    controller: &Retained<CafeController>,
    lang: Lang,
    mtm: MainThreadMarker,
) -> MenuParts {
    use objc2::runtime::AnyObject;

    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);

    // SAFETY: the controller outlives the menu (both held by AppState/the app
    // for the process lifetime); target/action and delegate are plain
    // non-owning references in AppKit.
    let controller_ref: &AnyObject = controller;
    let delegate = ProtocolObject::from_ref(&**controller);

    // Header.
    let header = NSMenuItem::new(mtm);
    header.setTitle(&NSString::from_str("☕ cafe"));
    header.setEnabled(false);
    menu.addItem(&header);

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Mode items.
    let mut mode_items = Vec::with_capacity(Mode::ALL.len());
    for mode in Mode::ALL {
        let item = action_item(mode.label(lang), tag_for(mode), mtm);
        unsafe {
            item.setTarget(Some(controller_ref));
            item.setAction(Some(sel!(selectAction:)));
        }
        item.setEnabled(true);
        menu.addItem(&item);
        mode_items.push(item);
    }

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Timed sessions.
    let mut timer_items = Vec::with_capacity(TIMER_PRESETS.len());
    for (i, mins) in TIMER_PRESETS.iter().enumerate() {
        let item = action_item(&timer_label(*mins, lang), TAG_TIMER_BASE + i as TagInt, mtm);
        unsafe {
            item.setTarget(Some(controller_ref));
            item.setAction(Some(sel!(selectAction:)));
        }
        item.setEnabled(true);
        menu.addItem(&item);
        timer_items.push(item);
    }

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Launch at login.
    let login_title = match lang {
        Lang::En => "Launch at Login",
        Lang::Zh => "登录时启动",
    };
    let login_item = action_item(login_title, TAG_LOGIN_ITEM, mtm);
    unsafe {
        login_item.setTarget(Some(controller_ref));
        login_item.setAction(Some(sel!(selectAction:)));
    }
    menu.addItem(&login_item);

    // Auto: watch agents.
    let auto_title = match lang {
        Lang::En => "Auto: Watch Agents",
        Lang::Zh => "自动:监测 Agent",
    };
    let auto_item = action_item(auto_title, TAG_AUTO_WATCH, mtm);
    unsafe {
        auto_item.setTarget(Some(controller_ref));
        auto_item.setAction(Some(sel!(selectAction:)));
    }
    menu.addItem(&auto_item);

    // Language toggle. The title always names the language you'd switch TO,
    // so it stays self-explanatory in either language.
    let lang_item = action_item(lang.toggle_label(), TAG_LANG_ITEM, mtm);
    unsafe {
        lang_item.setTarget(Some(controller_ref));
        lang_item.setAction(Some(sel!(selectAction:)));
    }
    menu.addItem(&lang_item);

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Quit.
    let quit_title = match lang {
        Lang::En => "Quit cafe",
        Lang::Zh => "退出 cafe",
    };
    let quit_item = NSMenuItem::new(mtm);
    quit_item.setTitle(&NSString::from_str(quit_title));
    unsafe {
        quit_item.setTarget(Some(controller_ref));
        quit_item.setAction(Some(sel!(quitAction:)));
    }
    menu.addItem(&quit_item);

    menu.setDelegate(Some(delegate));

    MenuParts {
        menu,
        mode_items,
        timer_items,
        login_item,
        auto_item,
        lang_item,
        quit_item,
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("cafe must run on the main thread");

    let app = NSApplication::sharedApplication(mtm);
    // Run as an accessory (menu bar only, no Dock icon).
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // Status bar item.
    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

    let mut state = AppState::new();
    let lang = state.lang;

    // Icon (color baked in per mode) + tooltip.
    if let Some(image) = state.icons.get(Mode::Off) {
        if let Some(button) = status_item.button(mtm) {
            button.setImage(Some(&image));
            button.setToolTip(Some(&NSString::from_str(Mode::Off.tooltip(lang))));
        }
    }

    // Controller.
    let controller = CafeController::new();

    // Menu.
    let parts = build_menu(&controller, lang, mtm);
    state.status_item = Some(status_item);
    state.mode_items = parts.mode_items;
    state.timer_items = parts.timer_items;
    state.login_item = Some(parts.login_item);
    state.auto_item = Some(parts.auto_item);
    state.lang_item = Some(parts.lang_item);
    state.quit_item = Some(parts.quit_item);

    // Wire the status item's menu.
    state
        .status_item
        .as_ref()
        .unwrap()
        .setMenu(Some(&parts.menu));

    let state = Rc::new(RefCell::new(state));

    // Hand the state to the controller via the RefCell ivar.
    *controller.ivars().state.borrow_mut() = Some(state.clone());

    // Countdown timer (1s) — created once, always running; the tick is a
    // no-op unless a deadline is set. It also consumes hotkey requests.
    let countdown = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            1.0,
            &controller,
            sel!(tickFire:),
            None,
            true,
        )
    };
    state.borrow_mut().countdown_timer = Some(countdown);

    // Agent-watch poll timer (5s).
    // SAFETY: as above.
    let agent = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            5.0,
            &controller,
            sel!(agentFire:),
            None,
            true,
        )
    };
    state.borrow_mut().agent_timer = Some(agent);

    // Global hotkey: Ctrl+Alt+C cycles Off → IdleOnly → IdleAndDisplay.
    // The hotkey event fires on a background thread; `Rc<RefCell>` state
    // can't cross threads, so the handler only sets an atomic flag and the
    // 1s main-thread countdown timer consumes it.
    let hotkey_manager = GlobalHotKeyManager::new().ok();
    if let Some(manager) = &hotkey_manager {
        let hk = HotKey::new(Some(Modifiers::ALT | Modifiers::CONTROL), Code::KeyC);
        if manager.register(hk).is_ok() {
            global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(
                move |e: global_hotkey::GlobalHotKeyEvent| {
                    if e.state() == global_hotkey::HotKeyState::Pressed {
                        HOTKEY_REQUEST.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                },
            ));
        }
    }

    // If auto-watch is enabled at launch, decide immediately.
    if state.borrow().auto_watch {
        let mut s = state.borrow_mut();
        let had_deadline = s.deadline.is_some();
        if !had_deadline {
            s.agents_were_running = agents_running();
            let target = if s.agents_were_running {
                Mode::IdleAndDisplay
            } else {
                Mode::Off
            };
            let _ = s.supervisor.enter(target, None);
            s.mode = target;
            s.refresh_ui(mtm);
        }
    }

    // Run the event loop. This blocks until the app terminates.
    autoreleasepool(|_| {
        app.run();
    });
}
