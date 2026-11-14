//! Tsundoku in the background: the tray icon and its menu, starting with Windows, the clipboard shortcut, pages sent
//! from a browser (`tsundoku://add?url=…`), and the notifications for links added, downloads that failed and a queue
//! that finished, while the window isn't in front.
//! Settings → Running in the Background and Adding Links turn them on; each is off until then, except notifications
//! and the browser's links.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, Url, Wry};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};
use tauri_plugin_notification::NotificationExt;
use tsundoku_core::data_folder;
use tsundoku_core::downloads::{
    DownloadError, DownloadQueue, QueueItem, RunEvent, WholeQueueAction, notices,
};
use tsundoku_core::listing::ListingKind;
use tsundoku_core::pending::{self, PendingLink, PendingLinks};
use tsundoku_core::presets::PresetManager;
use tsundoku_core::settings::{BackgroundSettings, ClipboardShortcut, SettingsStore};

use crate::focus_main_window;
use crate::queue_status::QueueStatus;

/// The argument Windows starts Tsundoku with at sign-in (Start with Windows), so it starts in the tray.
pub const IN_TRAY_ARG: &str = "--in-tray";

/// The name Windows lists under its startup apps. Development builds and portable copies keep their own, so turning
/// Start with Windows on or off in one leaves the installed app's alone.
pub fn startup_name() -> &'static str {
    if cfg!(debug_assertions) {
        "Tsundoku (Development)"
    } else if data_folder::portable_folder().is_some() {
        "Tsundoku (Portable)"
    } else {
        "Tsundoku"
    }
}

const TRAY_ID: &str = "main";
const OPEN_ID: &str = "open";
const WATCH_ID: &str = "watch-clipboard";
const START_ALL_ID: &str = "start-all";
const PAUSE_ALL_ID: &str = "pause-all";
const QUIT_ID: &str = "quit";

/// What the tray, the question before quitting and the notifications say. The UI's own texts are in strings.ts;
/// these show outside it, so they live here, in the same voice.
mod text {
    use tsundoku_core::listing::ListingKind;

    pub const TOOLTIP: &str = "Tsundoku";
    pub const OPEN: &str = "Open Tsundoku";
    pub const WATCH_CLIPBOARD: &str = "Watch Clipboard";
    pub const QUIT: &str = "Quit Tsundoku";

    pub fn start_all(count: usize) -> String {
        match count {
            0 => "Start All Pending".to_owned(),
            count => format!("Start All Pending ({count})"),
        }
    }

    pub const PAUSE_ALL: &str = "Pause All Downloads";
    pub const RESUME_ALL: &str = "Resume All Downloads";

    pub const QUIT_TITLE: &str = "Downloads Are Running";
    pub const QUIT_CONFIRM: &str = "Quit";
    pub const QUIT_CANCEL: &str = "Cancel";

    pub fn quit_message(count: usize) -> String {
        if count == 1 {
            "A download is still running. Quit Tsundoku anyway? Next time, it can continue where it stopped.".to_owned()
        } else {
            format!(
                "{count} downloads are still running. Quit Tsundoku anyway? Next time, they can continue where they \
                 stopped."
            )
        }
    }

    pub const DOWNLOADING: &str = "Downloading";
    pub const ADDED_TO_PENDING: &str = "Added to Pending";

    /// A copied link that lists several items, which waits in Pending's Needs Attention to be picked from.
    pub fn to_pick_from(listing: Option<ListingKind>, link: &str) -> String {
        let what = match listing {
            Some(ListingKind::Channel) => "A channel",
            Some(ListingKind::Playlist) => "A playlist",
            Some(ListingKind::Account) => "An account",
            None => "A page of several items",
        };
        format!("{what} to pick from: {link}")
    }
    pub const NOTHING_TO_ADD: &str = "Nothing to Add";
    pub const NO_LINKS: &str = "There's no link on the clipboard.";
    pub const STARTED: &str = "Started";

    pub const NO_LINKS_SENT: &str = "Your browser sent no link.";

    pub fn links_added(count: u32, from_browser: bool, right_away: bool) -> String {
        let links = if count == 1 {
            "A link".to_owned()
        } else {
            format!("{count} links")
        };
        let from = if from_browser {
            "your browser"
        } else {
            "the clipboard"
        };
        let then = match (right_away, count == 1) {
            (true, true) => " It starts once read.",
            (true, false) => " They start once read.",
            (false, _) => "",
        };
        format!("{links} from {from}.{then}")
    }

    pub fn already_pending(from_browser: bool) -> &'static str {
        if from_browser {
            "The page your browser sent is pending already."
        } else {
            "The links on the clipboard are pending already."
        }
    }

    /// A Click'n'Load button's links, by the site that sent them: "FileCrypt Sent 12 Links".
    pub fn page_sent(site: &str, count: u32) -> String {
        if count == 1 {
            format!("{site} Sent a Link")
        } else {
            format!("{site} Sent {count} Links")
        }
    }

    pub fn page_links_read(count: u32) -> &'static str {
        if count == 1 {
            "It's being read in Pending."
        } else {
            "They're being read in Pending."
        }
    }

    pub fn page_links_pending(site: &str) -> String {
        format!("The links {site} sent are pending already.")
    }

    pub fn links_started(count: usize) -> String {
        if count == 1 {
            "A pending link joined the queue.".to_owned()
        } else {
            format!("{count} pending links joined the queue.")
        }
    }
}

/// The tray menu's items that change while it shows: Watch Clipboard's tick, Start All Pending's count, and Pause All
/// Downloads, which is Resume All Downloads while everything is stopped. `None` while there's no tray icon.
#[derive(Default)]
pub struct TrayMenu {
    items: Mutex<Option<TrayItems>>,
}

struct TrayItems {
    watch: CheckMenuItem<Wry>,
    start_all: MenuItem<Wry>,
    pause_all: MenuItem<Wry>,
}

impl TrayMenu {
    fn with_items(&self, change: impl FnOnce(&TrayItems)) {
        if let Some(items) = self
            .items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            change(items);
        }
    }

    fn set_items(&self, items: Option<TrayItems>) {
        *self.items.lock().unwrap_or_else(PoisonError::into_inner) = items;
    }
}

/// Whether the window stays hidden at launch: the tray is on and Windows started Tsundoku, at sign-in or for a page
/// a browser sent.
pub struct StartedInTray(pub bool);

/// Sets up what runs in the background once the data is open: the tray icon if it's on, the clipboard shortcut if
/// one is picked, and opening the window when a notification is clicked.
pub fn start(app: &AppHandle, settings: &SettingsStore) {
    let saved = settings.get();
    let in_tray = saved.background.keep_in_tray;
    let sent = app.deep_link().get_current().ok().flatten();
    app.manage(StartedInTray(
        in_tray && (sent.is_some() || std::env::args().any(|arg| arg == IN_TRAY_ARG)),
    ));
    app.manage(TrayMenu::default());
    app.manage(GatheredEvents::default());
    if in_tray {
        // Without a tray icon the window still opens and closes as before, so a failure here isn't worth stopping
        // the app for.
        let _ = show_tray(app, true);
    }
    if let Some(shortcut) = saved.adding.clipboard_shortcut {
        // Another app may have taken the keys since they were picked; Settings says so when they're picked again.
        let _ = app.global_shortcut().register(shortcut_keys(shortcut));
    }
    let handle = app.clone();
    let _ = app
        .notification()
        .on_action(move |_| focus_main_window(&handle));

    // Pages a browser sends: the one Tsundoku was started for, then those handed to it while it runs (the single
    // instance plugin passes a second launch's link on). Registering again each start keeps the browser bookmark
    // working if Tsundoku moved; the installer registers it too.
    let _ = app.deep_link().register_all();
    if let Some(urls) = sent {
        add_sent_from_browser(app, &urls);
    }
    let handle = app.clone();
    app.deep_link()
        .on_open_url(move |event| add_sent_from_browser(&handle, &event.urls()));
}

/// Whether a launch's arguments are a page a browser sent, which adds it without bringing the window forward.
pub fn is_sent_link(args: &[String]) -> bool {
    args.iter().any(|arg| {
        arg.get(..pending::SEND_SCHEME.len() + 1)
            .is_some_and(|start| start.eq_ignore_ascii_case(&format!("{}:", pending::SEND_SCHEME)))
    })
}

fn add_sent_from_browser(app: &AppHandle, urls: &[Url]) {
    let links: Vec<String> = urls
        .iter()
        .flat_map(|url| pending::links_in_sent_link(url.as_str()))
        .collect();
    let app = app.clone();
    // Off the main thread, as the shortcut's links are: saving them may wait for the pending links' lock.
    thread::spawn(move || add_sent(&app, &links, true));
}

/// Settings → Running in the Background.
#[tauri::command]
pub fn background_options(settings: State<'_, Arc<SettingsStore>>) -> BackgroundSettings {
    settings.get().background
}

/// Saves Settings → Running in the Background, adding or removing the tray icon and Tsundoku's entry among Windows'
/// startup apps.
#[tauri::command]
pub fn save_background_options(
    app: AppHandle,
    settings: State<'_, Arc<SettingsStore>>,
    options: BackgroundSettings,
) -> Result<BackgroundSettings, DownloadError> {
    let previous = settings.get().background;
    if options.start_with_windows != previous.start_with_windows {
        let startup = app.autolaunch();
        let changed = if options.start_with_windows {
            startup.enable()
        } else {
            startup.disable()
        };
        changed.map_err(|error| DownloadError::Unexpected {
            detail: error.to_string(),
        })?;
    }
    settings
        .update(|settings| settings.background = options)
        .map_err(|error| DownloadError::Storage {
            detail: error.to_string(),
        })?;
    if options.keep_in_tray != previous.keep_in_tray {
        show_tray(&app, options.keep_in_tray).map_err(|error| DownloadError::Unexpected {
            detail: error.to_string(),
        })?;
    }
    Ok(options)
}

/// Adds or removes the tray icon.
fn show_tray(app: &AppHandle, show: bool) -> tauri::Result<()> {
    let tray_menu = app.state::<TrayMenu>();
    if !show {
        tray_menu.set_items(None);
        app.remove_tray_by_id(TRAY_ID);
        return Ok(());
    }
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }
    let pending = app.state::<Arc<PendingLinks>>();
    let startable = startable_count(&pending.list());
    let open = MenuItem::with_id(app, OPEN_ID, text::OPEN, true, None::<&str>)?;
    let watch = CheckMenuItem::with_id(
        app,
        WATCH_ID,
        text::WATCH_CLIPBOARD,
        true,
        pending.options().watch_clipboard,
        None::<&str>,
    )?;
    let start_all = MenuItem::with_id(
        app,
        START_ALL_ID,
        text::start_all(startable),
        startable > 0,
        None::<&str>,
    )?;
    let whole_queue = app.state::<Arc<DownloadQueue>>().whole_queue_action();
    let pause_all = MenuItem::with_id(
        app,
        PAUSE_ALL_ID,
        pause_all_text(whole_queue),
        whole_queue.is_some(),
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, QUIT_ID, text::QUIT, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &watch,
            &start_all,
            &pause_all,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    let tooltip = app
        .try_state::<Arc<QueueStatus>>()
        .and_then(|status| status.tooltip());
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tooltip.as_deref().unwrap_or(text::TOOLTIP))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu_item(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                focus_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    tray_menu.set_items(Some(TrayItems {
        watch,
        start_all,
        pause_all,
    }));
    Ok(())
}

fn on_menu_item(app: &AppHandle, id: &str) {
    match id {
        OPEN_ID => focus_main_window(app),
        WATCH_ID => {
            let pending = app.state::<Arc<PendingLinks>>();
            let options = pending.options();
            // The tick follows what's saved (`watching_changed`), whether or not saving worked.
            let _ = pending.set_options(tsundoku_core::settings::AddingSettings {
                watch_clipboard: !options.watch_clipboard,
                ..options
            });
            watching_changed(app, pending.options().watch_clipboard);
        }
        START_ALL_ID => start_all_pending(app),
        PAUSE_ALL_ID => pause_or_resume_all(app),
        QUIT_ID => quit(app),
        _ => {}
    }
}

/// Pause All Downloads, or Resume All Downloads while everything is stopped: what the item says once the queue has
/// told the menu, worked out again in case it changed since. A failure to save shows in the Queue view's rows.
fn pause_or_resume_all(app: &AppHandle) {
    let queue = app.state::<Arc<DownloadQueue>>();
    let _ = match queue.whole_queue_action() {
        Some(WholeQueueAction::PauseAll) => queue.stop_all(),
        Some(WholeQueueAction::ResumeAll) => queue.resume_all(),
        None => Ok(()),
    };
}

fn pause_all_text(action: Option<WholeQueueAction>) -> &'static str {
    match action {
        Some(WholeQueueAction::ResumeAll) => text::RESUME_ALL,
        Some(WholeQueueAction::PauseAll) | None => text::PAUSE_ALL,
    }
}

/// The tray icon's tooltip: where the queue's run stands, or `None` for the app's name. On the main thread, since the
/// tray waits for it otherwise (`queue_status`).
pub fn set_tray_tooltip(app: &AppHandle, tooltip: Option<&str>) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip.unwrap_or(text::TOOLTIP)));
    }
}

/// Keeps Pause All Downloads current: Resume All Downloads while everything is stopped, off with nothing to do.
pub fn queue_changed(app: &AppHandle, items: &[QueueItem]) {
    let action = WholeQueueAction::for_items(items);
    if let Some(tray_menu) = app.try_state::<TrayMenu>() {
        tray_menu.with_items(|tray_items| {
            let _ = tray_items.pause_all.set_text(pause_all_text(action));
            let _ = tray_items.pause_all.set_enabled(action.is_some());
        });
    }
}

/// Start All Pending: the same links the Pending view's Start All takes, with the default preset.
fn start_all_pending(app: &AppHandle) {
    let default_id = app.state::<Arc<PresetManager>>().overview().default_id;
    if let Ok(count) = app.state::<Arc<PendingLinks>>().start_all(&default_id)
        && count > 0
    {
        announce(app, text::STARTED, &text::links_started(count));
    }
}

/// Quits, first asking while downloads run, as closing the window does without the tray.
fn quit(app: &AppHandle) {
    let running = app.state::<Arc<DownloadQueue>>().running_count();
    if running == 0 {
        app.exit(0);
        return;
    }
    focus_main_window(app);
    let handle = app.clone();
    app.dialog()
        .message(text::quit_message(running))
        .title(text::QUIT_TITLE)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            text::QUIT_CONFIRM.to_owned(),
            text::QUIT_CANCEL.to_owned(),
        ))
        .show(move |confirmed| {
            if confirmed {
                handle.exit(0);
            }
        });
}

/// How many pending links Start All Pending would start.
fn startable_count(links: &[PendingLink]) -> usize {
    links
        .iter()
        .filter(|link| pending::starts_with_all(link))
        .count()
}

/// Keeps Start All Pending's count current.
pub fn pending_changed(app: &AppHandle, links: &[PendingLink]) {
    let count = startable_count(links);
    if let Some(tray_menu) = app.try_state::<TrayMenu>() {
        tray_menu.with_items(|items| {
            let _ = items.start_all.set_text(text::start_all(count));
            let _ = items.start_all.set_enabled(count > 0);
        });
    }
}

/// Keeps Watch Clipboard's tick current, whether Settings or the tray turned the watcher on or off.
pub fn watching_changed(app: &AppHandle, watching: bool) {
    if let Some(tray_menu) = app.try_state::<TrayMenu>() {
        tray_menu.with_items(|items| {
            let _ = items.watch.set_checked(watching);
        });
    }
}

/// The keys Windows is asked for.
fn shortcut_keys(shortcut: ClipboardShortcut) -> Shortcut {
    let (modifiers, key) = match shortcut {
        ClipboardShortcut::CtrlAltD => (Modifiers::CONTROL | Modifiers::ALT, Code::KeyD),
        ClipboardShortcut::CtrlShiftD => (Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyD),
        ClipboardShortcut::CtrlAltV => (Modifiers::CONTROL | Modifiers::ALT, Code::KeyV),
        ClipboardShortcut::AltShiftD => (Modifiers::ALT | Modifiers::SHIFT, Code::KeyD),
    };
    Shortcut::new(Some(modifiers), key)
}

/// Swaps the clipboard shortcut for another, or for none. When Windows won't give the new keys (another app has
/// them), the old shortcut stays and the error says so.
pub fn change_clipboard_shortcut(
    app: &AppHandle,
    from: Option<ClipboardShortcut>,
    to: Option<ClipboardShortcut>,
) -> Result<(), DownloadError> {
    if from == to {
        return Ok(());
    }
    let shortcuts = app.global_shortcut();
    if let Some(from) = from {
        let _ = shortcuts.unregister(shortcut_keys(from));
    }
    let Some(to) = to else {
        return Ok(());
    };
    if shortcuts.register(shortcut_keys(to)).is_err() {
        if let Some(from) = from {
            let _ = shortcuts.register(shortcut_keys(from));
        }
        return Err(DownloadError::ShortcutTaken {
            keys: to.keys().to_owned(),
        });
    }
    Ok(())
}

/// The global shortcut plugin's handler: pressing the clipboard shortcut adds the links on the clipboard to the
/// pending links, as if pasted there.
pub fn on_shortcut(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    // Off the main thread: opening the clipboard may wait for another app to close it.
    let app = app.clone();
    thread::spawn(move || add_copied_links(&app));
}

fn add_copied_links(app: &AppHandle) {
    #[cfg(windows)]
    let text = tsundoku_core::clipboard::copied_text().unwrap_or_default();
    #[cfg(not(windows))]
    let text = String::new();
    add_sent(app, &pending::links_in(&text), false);
}

/// Adds links sent from outside the window, the clipboard shortcut's or a browser's, and says what came of it.
fn add_sent(app: &AppHandle, links: &[String], from_browser: bool) {
    if links.is_empty() {
        let why = if from_browser {
            text::NO_LINKS_SENT
        } else {
            text::NO_LINKS
        };
        announce(app, text::NOTHING_TO_ADD, why);
        return;
    }
    let pending = app.state::<Arc<PendingLinks>>();
    let right_away = pending.options().download_right_away;
    match pending.add_sent(links) {
        Ok(added) if added.added > 0 => announce(
            app,
            text::ADDED_TO_PENDING,
            &text::links_added(added.added, from_browser, right_away),
        ),
        Ok(_) => announce(
            app,
            text::NOTHING_TO_ADD,
            text::already_pending(from_browser),
        ),
        // Saving failed; the Pending view shows nothing new, and pasting there tells why.
        Err(_) => {}
    }
}

/// A page's Click'n'Load button sent links, `added` of them new to Pending: the user hears of it unless the window is
/// in front.
pub fn page_sent(app: &AppHandle, site: &str, added: u32) {
    if added > 0 {
        announce(
            app,
            &text::page_sent(site, added),
            text::page_links_read(added),
        );
    } else {
        announce(app, text::NOTHING_TO_ADD, &text::page_links_pending(site));
    }
}

/// A link copied in another app was read and stays: the user hears of it unless the window is in front, where the
/// Pending view shows it.
pub fn copied_link_read(app: &AppHandle, title: &str, started: bool) {
    let heading = if started {
        text::DOWNLOADING
    } else {
        text::ADDED_TO_PENDING
    };
    announce(app, heading, title);
}

/// A link copied in another app lists several items and waits in Pending to be picked from: the user hears of it
/// unless the window is in front, where the Pending view shows it.
pub fn copied_link_to_pick(app: &AppHandle, link: &str, listing: Option<ListingKind>) {
    announce(
        app,
        text::ADDED_TO_PENDING,
        &text::to_pick_from(listing, link),
    );
}

/// A Windows notification about links added, when Settings → Adding Links has them on and the window isn't in front.
fn announce(app: &AppHandle, title: &str, body: &str) {
    if app.state::<Arc<PendingLinks>>().options().notify {
        tell(app, title, body);
    }
}

/// A Windows notification, unless the window is in front.
fn tell(app: &AppHandle, title: &str, body: &str) {
    if window_in_front(app) {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

/// How long the queue's run events are gathered before they're told, so failures that come together make one
/// notification, followed by the run's end if it came with them.
const GATHERING_TIME: Duration = Duration::from_millis(1500);

/// The queue's run events waiting to be told (`queue_events`).
#[derive(Default)]
pub struct GatheredEvents {
    events: Mutex<Vec<RunEvent>>,
}

/// A download failed or the queue's run ended: told after a moment, with whatever else happens meanwhile, as
/// Settings → Adding Links asks, while the window isn't in front.
pub fn queue_events(app: &AppHandle, events: Vec<RunEvent>) {
    let Some(gathered) = app.try_state::<GatheredEvents>() else {
        return;
    };
    let mut waiting = gathered
        .events
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let first = waiting.is_empty();
    waiting.extend(events);
    if !first || waiting.is_empty() {
        return;
    }
    drop(waiting);
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(GATHERING_TIME);
        let events = std::mem::take(
            &mut *app
                .state::<GatheredEvents>()
                .events
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        let options = app.state::<Arc<PendingLinks>>().options();
        for notice in notices(&events, options.notify_failures, options.notify_finished) {
            tell(&app, &notice.title, &notice.body);
        }
    });
}

fn window_in_front(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|window| {
        window.is_visible().unwrap_or(false)
            && !window.is_minimized().unwrap_or(true)
            && window.is_focused().unwrap_or(false)
    })
}
