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
    NSAlert, NSApplication, NSApplicationActivationPolicy, NSCellImagePosition,
    NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar,
    NSStatusItem, NSTextField, NSVariableStatusItemLength,
};
use objc2_foundation::{NSDate, NSSize, NSString, NSTimeInterval, NSTimer};

mod icon;
mod state;
mod supervisor;

use state::{
    countdown_text, detect_agents, format_clock, load_config, parse_custom_minutes, save_config,
    set_login_item, timer_label, timer_target, Config, Lang, Mode, AGENTS, TIMER_PRESETS,
};
use supervisor::Supervisor;

/// Menu item tag encoding (see `tag_for` / `mode_for_tag`).
type TagInt = isize;

/// Tag namespaces so the selector knows what kind of item fired.
const TAG_MODE_BASE: TagInt = 100; // 100..=102 → modes

/// Background agent-scan cadence in seconds. Opening the menu always scans
/// on demand, so this only feeds the auto-watch decision latency.
const AGENT_POLL_INTERVAL: f64 = 60.0;
const TAG_AGENT_BASE: TagInt = 150; // 150+i → AGENTS[i] (display-only slots)
const TAG_TIMER_BASE: TagInt = 200; // 200+i → TIMER_PRESETS[i]
const TAG_TIMER_CUSTOM: TagInt = 210; // "Custom…" → NSAlert input
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
    /// One menu item per entry in `state::AGENTS` (lit when its process is
    /// running, hidden otherwise).
    agent_items: Vec<Retained<NSMenuItem>>,
    /// Last observed online state per `state::AGENTS`.
    agents_online: Vec<bool>,
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
    /// The disabled header line (shows "☕ cafe — 12:34" while timed).
    header_item: Option<Retained<NSMenuItem>>,
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
    /// Pre-rendered online dots for the agent rows.
    agent_icons: icon::AgentIcons,
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
            agent_items: Vec::new(),
            agents_online: vec![false; AGENTS.len()],
            timer_items: Vec::new(),
            login_item: None,
            auto_item: None,
            lang_item: None,
            quit_item: None,
            header_item: None,
            last_spawn_error: None,
            deadline: None,
            countdown_timer: None,
            agent_timer: None,
            agents_were_running: false,
            auto_watch: config.auto_watch,
            lang: config.lang,
            icons: icon::IconCache::new(),
            agent_icons: icon::AgentIcons::new(),
            config,
        }
    }

    /// One process-table scan feeds BOTH the online lights and auto mode.
    fn sync_agents(&mut self, mtm: MainThreadMarker) {
        let online = detect_agents();
        self.agents_online = online.clone();
        self.refresh_ui(mtm);
        self.auto_decide(online.iter().any(|&b| b), mtm);
    }

    /// Persist the config's mutable bits from live state. `last_mode` only
    /// tracks *armed* modes, so it survives going Off and stays a useful
    /// default for timed sessions started from Off.
    fn persist(&mut self) {
        if self.mode != Mode::Off {
            self.config.last_mode = self.mode;
        }
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

    /// Arm a timed session for `minutes`, then auto-revert to Off.
    ///
    /// The timer applies to the **currently selected** feature: an already
    /// armed mode keeps running and just gets a deadline; starting from Off
    /// arms the last manually used mode (or Idle Only if never armed).
    fn apply_timer(&mut self, minutes: u64, mtm: MainThreadMarker) {
        // A timed session is a manual action; cancel auto-watch.
        self.auto_watch = false;
        let target = timer_target(self.mode, self.config.last_mode);
        let secs = minutes * 60;
        match self.supervisor.enter(target, Some(secs)) {
            Ok(_) => {
                self.mode = target;
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
    fn auto_decide(&mut self, running: bool, mtm: MainThreadMarker) {
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
            let online = detect_agents();
            self.agents_online = online.clone();
            let running = online.iter().any(|&b| b);
            self.agents_were_running = running;
            let target = if running {
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

    /// Sync every UI surface with current state: icon + countdown clock,
    /// tooltip, check marks, localized titles, error line.
    fn refresh_ui(&mut self, mtm: MainThreadMarker) {
        let lang = self.lang;

        // Icon + live clock text + tooltip. When a timed session runs, the
        // remaining h:mm:ss renders right next to the cup so it ticks without
        // opening the menu.
        if let Some(item) = &self.status_item {
            if let Some(button) = item.button(mtm) {
                if let Some(img) = self.icons.get(self.mode) {
                    button.setImage(Some(&img));
                }
                button.setImagePosition(NSCellImagePosition::ImageLeading);
                let clock = self.countdown_clock();
                button.setTitle(&NSString::from_str(&clock));
                let tip = self.status_tooltip();
                button.setToolTip(Some(&NSString::from_str(&tip)));
            }
        }
        // Menu header mirrors the same info for when the menu is open.
        if let Some(item) = &self.header_item {
            let clock = self.countdown_clock();
            let title = if clock.is_empty() {
                "☕ cafe".to_string()
            } else {
                format!("☕ cafe — {clock}")
            };
            item.setTitle(&NSString::from_str(&title));
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
        // Agent rows: lit dot + name when the process is running, hidden when
        // it is not ("没有亮的就是没有启动,也不显示").
        for (i, item) in self.agent_items.iter().enumerate() {
            let online = self.agents_online.get(i).copied().unwrap_or(false);
            let label = state::AGENTS.get(i).map(|(_, l)| *l).unwrap_or("");
            item.setTitle(&NSString::from_str(label));
            if online {
                if let Some(dot) = self.agent_icons.get(i) {
                    item.setImage(Some(&dot));
                }
                item.setHidden(false);
            } else {
                item.setHidden(true);
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

    /// Live clock text for the menu bar / header: `m:ss` or `h:mm:ss` while a
    /// timed session runs, empty otherwise.
    fn countdown_clock(&self) -> String {
        self.deadline
            .map(|d| {
                let now = NSDate::now().timeIntervalSinceReferenceDate();
                let secs = (d - now).max(0.0) as u64;
                format_clock(secs)
            })
            .unwrap_or_default()
    }

    /// Tick: called by the countdown timer every second (in common modes, so
    /// it also fires while a menu is open). Updates the visible countdown;
    /// disarms when the deadline passes.
    fn tick_countdown(&mut self, mtm: MainThreadMarker) {
        let Some(d) = self.deadline else { return };
        if NSDate::now().timeIntervalSinceReferenceDate() >= d {
            self.deadline = None;
            let _ = self.supervisor.enter(Mode::Off, None);
            self.mode = Mode::Off;
            self.refresh_ui(mtm);
            return;
        }
        // Cheap per-second update: clock text + tooltip + header.
        let clock = self.countdown_clock();
        let tip = self.status_tooltip();
        if let Some(item) = &self.status_item {
            if let Some(button) = item.button(mtm) {
                button.setTitle(&NSString::from_str(&clock));
                button.setToolTip(Some(&NSString::from_str(&tip)));
            }
        }
        if let Some(item) = &self.header_item {
            let title = format!("☕ cafe — {clock}");
            item.setTitle(&NSString::from_str(&title));
        }
    }
}

/// "Custom…" timer: modal input dialog asking for minutes. Returns the parsed
/// duration, or `None` on cancel/invalid input.
///
/// IMPORTANT: this is a free function and must be called WITHOUT holding a
/// borrow of the shared `AppState` — `runModal` spins a modal run loop in
/// which our 1-second tick timer fires (`tickFire:` borrows the state), so
/// any live borrow here would panic with `BorrowMutError`.
fn prompt_custom_minutes(lang: Lang, mtm: MainThreadMarker) -> Option<u64> {
    let (title, info, placeholder, button) = match lang {
        Lang::En => (
            "Custom timed session",
            "Keep awake for how many minutes? (1–1440)",
            "e.g. 45",
            "Start",
        ),
        Lang::Zh => ("自定义定时", "保持唤醒多少分钟?(1–1440)", "例如 45", "开始"),
    };
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(info));

    let field = NSTextField::textFieldWithString(&NSString::from_str(""), mtm);
    // `textFieldWithString:` creates the field with a tiny default frame and
    // NSAlert does not resize accessory views — give it an explicit, usable
    // size or the input renders as a sliver.
    field.setFrameSize(NSSize::new(280.0, 24.0));
    field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
    // SAFETY: field coerces to its NSView superclass for the accessory.
    alert.setAccessoryView(Some(&field));
    alert.addButtonWithTitle(&NSString::from_str(button));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));

    let response = alert.runModal();
    // The first button returns NSAlertFirstButtonReturn (1000).
    if response != 1000 {
        return None;
    }
    let text = field.stringValue().to_string();
    parse_custom_minutes(&text)
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
            } else if tag == TAG_TIMER_CUSTOM {
                // Run the modal dialog WITHOUT holding a state borrow: the
                // tick timer fires inside `runModal`'s run loop and borrows
                // the state (see `prompt_custom_minutes`).
                let lang = state.borrow().lang;
                if let Some(minutes) = prompt_custom_minutes(lang, mtm) {
                    state.borrow_mut().apply_timer(minutes, mtm);
                }
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

        /// NSTimer target for the agent-watch poll (1 min). One process-table
        /// scan feeds both the online lights and auto mode.
        #[unsafe(method(agentFire:))]
        fn agent_fire(&self, _sender: *mut NSObject) {
            let mtm = MainThreadMarker::new().expect("timer on main thread");
            let state = self.ivars().state.borrow().clone();
            let Some(state) = state else { return };
            state.borrow_mut().sync_agents(mtm);
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
            // Fresh agent scan so the lights are accurate while the user
            // watches (the background poll can lag up to AGENT_POLL_INTERVAL).
            s.sync_agents(mtm);
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
    header_item: Retained<NSMenuItem>,
    mode_items: Vec<Retained<NSMenuItem>>,
    agent_items: Vec<Retained<NSMenuItem>>,
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

    // Agent slots: one per state::AGENTS entry, hidden until its process is
    // detected. They live inside the mode group so the layout is identical
    // when no agent is online (no stray separators).
    let mut agent_items = Vec::with_capacity(AGENTS.len());
    for (name, label) in AGENTS {
        let item = action_item(label, TAG_AGENT_BASE + agent_items.len() as TagInt, mtm);
        // Agents rows are status displays, not actions.
        item.setEnabled(false);
        item.setHidden(true);
        let _ = name;
        menu.addItem(&item);
        agent_items.push(item);
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

    // Custom duration ("Custom…" / "自定义…").
    let custom_title = match lang {
        Lang::En => "Custom…",
        Lang::Zh => "自定义…",
    };
    let custom_item = action_item(custom_title, TAG_TIMER_CUSTOM, mtm);
    unsafe {
        custom_item.setTarget(Some(controller_ref));
        custom_item.setAction(Some(sel!(selectAction:)));
    }
    custom_item.setEnabled(true);
    menu.addItem(&custom_item);

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
        header_item: header,
        mode_items,
        agent_items,
        timer_items,
        login_item,
        auto_item,
        lang_item,
        quit_item,
    }
}

fn main() {
    // A menu bar app has no visible stderr when launched via `open`; route
    // panics into a log file so any future crash is diagnosable.
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("cafe panicked: {info}");
        eprintln!("{msg}");
        state::append_panic_log(&msg);
    }));

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
    state.agent_items = parts.agent_items;
    state.timer_items = parts.timer_items;
    state.login_item = Some(parts.login_item);
    state.auto_item = Some(parts.auto_item);
    state.lang_item = Some(parts.lang_item);
    state.quit_item = Some(parts.quit_item);
    state.header_item = Some(parts.header_item);

    // Wire the status item's menu.
    state
        .status_item
        .as_ref()
        .unwrap()
        .setMenu(Some(&parts.menu));

    let state = Rc::new(RefCell::new(state));

    // Hand the state to the controller via the RefCell ivar.
    *controller.ivars().state.borrow_mut() = Some(state.clone());

    // Countdown timer (1s), added to NSRunLoopCommonModes so it fires even
    // while a menu is open — the visible countdown must tick with the menu
    // showing. Created unscheduled, then registered manually. The tick is a
    // no-op unless a deadline is set; it also consumes hotkey requests.
    // SAFETY: standard AppKit timer wiring on the main thread run loop.
    let countdown = unsafe {
        let timer = NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
            1.0,
            &controller,
            sel!(tickFire:),
            None,
            true,
        );
        let run_loop = objc2_foundation::NSRunLoop::currentRunLoop();
        run_loop.addTimer_forMode(&timer, objc2_foundation::NSRunLoopCommonModes);
        timer
    };
    state.borrow_mut().countdown_timer = Some(countdown);

    // Agent-watch poll timer (1 minute default; opening the menu always
    // triggers a fresh scan, so the lights are accurate when actually seen).
    // SAFETY: as above.
    let agent = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            AGENT_POLL_INTERVAL,
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

    // Initial agent scan: lights the online rows (and arms auto-watch if the
    // config has it on) before the event loop starts.
    state.borrow_mut().sync_agents(mtm);

    // Run the event loop. This blocks until the app terminates.
    autoreleasepool(|_| {
        app.run();
    });
}
