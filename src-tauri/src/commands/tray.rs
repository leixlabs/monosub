//! The menu-bar (tray) item: what it displays, and what a left click shows.
//!
//! The item used to carry a fixed Show / Hide / Quit menu, and a left click
//! toggled the window. It now answers "what is the app doing?": a left click
//! opens a menu whose first rows are the current transcription progress, and the
//! menu-bar title carries the percentage while a run is in flight.
//!
//! Two commands write into one [`TrayState`], and every write re-renders the whole
//! menu from that snapshot:
//!
//! - [`update_tray_menu`] carries the localised Show / Hide / Quit labels.
//! - [`update_tray_progress`] carries the progress rows and the menu-bar text.
//!
//! Rendering from a single snapshot is what stops the two from fighting: a
//! language change must not wipe the progress rows, and a progress update must
//! not revert the labels to English.
//!
//! The **wording** lives in the frontend (`src/lib/tray.ts` plus the `tray.*`
//! i18n keys), because that is the only place that knows the active locale. This
//! module renders what it is given and owns only the layout rules: at most
//! [`MAX_STATUS_ROWS`] rows, no blank rows, and a row longer than
//! [`MAX_ROW_CHARS`] truncated — a long file name would otherwise stretch the
//! menu across the screen.

use serde::Deserialize;
use std::sync::Mutex;
use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    AppHandle, Runtime,
};

/// How many progress rows sit above the separator.
///
/// Every extra row pushes Show / Hide / Quit further down and makes the menu
/// taller than the information is worth; the current run, its step and the queue
/// behind it are the three things that matter.
pub const MAX_STATUS_ROWS: usize = 3;

/// Longest row, in **characters** (not bytes).
pub const MAX_ROW_CHARS: usize = 60;

/// Localised menu labels. The frontend pushes these on mount and on every
/// language change; the defaults keep the menu readable in the window between the
/// tray appearing and the first push.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayLabels {
    show: String,
    hide: String,
    quit: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            show: "Show Window".to_string(),
            hide: "Hide Window".to_string(),
            quit: "Quit".to_string(),
        }
    }
}

/// One push from the frontend: what the menu bar should say right now.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrayProgress {
    /// Menu-bar text next to the icon, e.g. `42%`. Absent (or blank) means the
    /// icon stands alone, which is the idle state.
    #[serde(default)]
    pub title: Option<String>,
    /// Greyed-out rows above the separator: the current file, its step, the queue.
    #[serde(default)]
    pub rows: Vec<String>,
}

/// Everything the tray renders, so one write cannot lose the other's contribution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TrayState {
    labels: TrayLabels,
    title: Option<String>,
    rows: Vec<String>,
}

impl TrayState {
    fn set_labels(&mut self, show: String, hide: String, quit: String) {
        self.labels = TrayLabels { show, hide, quit };
    }

    fn set_progress(&mut self, progress: TrayProgress) {
        self.title = normalize_title(progress.title);
        self.rows = normalize_rows(progress.rows);
    }
}

lazy_static::lazy_static! {
    static ref STATE: Mutex<TrayState> = Mutex::new(TrayState::default());
}

/// The id the tray is built with in `crate::tray::create_tray`.
const TRAY_ID: &str = "main-tray";

/// A blank title means "no menu-bar text", never an empty label.
fn normalize_title(title: Option<String>) -> Option<String> {
    let title = title?;
    let title = title.trim();
    if title.is_empty() {
        None
    } else {
        Some(truncate(title, MAX_ROW_CHARS))
    }
}

/// Trim, drop blanks, keep the first [`MAX_STATUS_ROWS`] and truncate what is too
/// wide.
///
/// A blank row would render as an empty greyed line, which reads as a rendering
/// bug rather than as "nothing here".
fn normalize_rows<I, S>(rows: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    rows.into_iter()
        .map(|row| row.as_ref().trim().to_string())
        .filter(|row| !row.is_empty())
        .take(MAX_STATUS_ROWS)
        .map(|row| truncate(&row, MAX_ROW_CHARS))
        .collect()
}

/// Truncate by characters, not bytes: a Chinese title cut on a byte boundary
/// would panic.
fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// One rendered menu row, before it becomes a `MenuItem` or a separator.
///
/// Splitting this out of [`refresh`] is what makes the *layout* testable: the
/// Tauri part is then a mechanical translation of this list. `enabled` is carried
/// here rather than decided in `refresh` so the tests can pin it — a progress row
/// that was clickable would be a menu item that looks like a command and is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowKind {
    /// A progress line: a display, so it must not be clickable.
    Status,
    Separator,
    /// An actionable line (`show` / `hide` / `quit`).
    Action,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    kind: RowKind,
    /// Stable per position, so a re-render replaces rather than accumulates.
    id: String,
    text: String,
}

impl Row {
    /// Whether the row is clickable. The single place that answers it, so the
    /// menu and the tests cannot disagree.
    fn enabled(&self) -> bool {
        self.kind == RowKind::Action
    }
}

/// The rows a menu shows, top to bottom.
///
/// Three rules, all of them things a user would notice:
///
/// - Status rows come first, so a left click answers with progress rather than
///   with actions.
/// - The separator appears **only when there is something above it**; an idle
///   menu whose first row is a separator is a stray line.
/// - The actions are always last and always the same three, in the same order, so
///   the muscle memory of "Quit is the bottom row" survives every update.
fn layout(state: &TrayState) -> Vec<Row> {
    let mut rows: Vec<Row> = state
        .rows
        .iter()
        .enumerate()
        .map(|(index, text)| Row {
            kind: RowKind::Status,
            id: format!("tray-status-{index}"),
            text: text.clone(),
        })
        .collect();

    if !rows.is_empty() {
        rows.push(Row {
            kind: RowKind::Separator,
            id: "tray-separator".to_string(),
            text: String::new(),
        });
    }

    for (id, text) in [
        ("show", state.labels.show.clone()),
        ("hide", state.labels.hide.clone()),
        ("quit", state.labels.quit.clone()),
    ] {
        rows.push(Row {
            kind: RowKind::Action,
            id: id.to_string(),
            text,
        });
    }

    rows
}

/// Re-render the tray from [`STATE`].
///
/// Build the menu described by [`layout`].
///
/// Split out of [`refresh`] so a test can build the **real** `Menu` — a mock
/// runtime never has a tray to look up — and assert on the items it actually
/// produced rather than on the intermediate enum.
fn build_menu<R: Runtime>(app: &AppHandle<R>, state: &TrayState) -> Result<Menu<R>, String> {
    // Built as concrete menu items first, because `Menu::with_items` wants
    // `&dyn IsMenuItem` references that outlive the call.
    let mut items: Vec<Box<dyn IsMenuItem<R>>> = Vec::new();
    for row in layout(state) {
        // Both flags are read BEFORE the fields move into the builder.
        let enabled = row.enabled();
        // `row.enabled()` is the single source of the enabled flag: a status row
        // is a display, so clicking it must not hide the menu or be mistaken for
        // a command.
        let item: Box<dyn IsMenuItem<R>> = if row.kind == RowKind::Separator {
            Box::new(PredefinedMenuItem::separator(app).map_err(|error| error.to_string())?)
        } else {
            Box::new(
                MenuItem::with_id(app, row.id, row.text, enabled, None::<&str>)
                    .map_err(|error| error.to_string())?,
            )
        };
        items.push(item);
    }

    let refs: Vec<&dyn IsMenuItem<R>> = items.iter().map(|item| item.as_ref()).collect();
    Menu::with_items(app, &refs).map_err(|error| error.to_string())
}

/// Called by both commands and once by `create_tray`, so there is exactly one
/// menu-building path and no way for a label push and a progress push to produce
/// different layouts.
pub(crate) fn refresh<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        // A missing tray is not a failure: the frontend pushes on mount, and a
        // cosmetic update must never turn into an error it has to handle.
        return Ok(());
    };

    let state = STATE
        .lock()
        .map_err(|_| "the tray state is poisoned".to_string())?
        .clone();

    let menu = build_menu(app, &state)?;
    tracing::debug!(
        title = state.title.as_deref().unwrap_or("-"),
        rows = state.rows.len(),
        show = %state.labels.show,
        "rendering the tray menu"
    );
    tray.set_menu(Some(menu))
        .map_err(|error| error.to_string())?;
    // The menu-bar text is the percentage while a run is in flight and nothing at
    // all when idle — a permanent `0%` is noise in the menu bar.
    tray.set_title(state.title.as_deref())
        .map_err(|error| error.to_string())?;

    Ok(())
}

/// Update the Show / Hide / Quit labels.
///
/// Progress rows are deliberately untouched: a language change while a file is
/// transcribing must not erase the thing the user opened the menu to see.
#[tauri::command]
pub async fn update_tray_menu(
    app: AppHandle,
    show: String,
    hide: String,
    quit: String,
) -> Result<(), String> {
    {
        let mut state = STATE
            .lock()
            .map_err(|_| "the tray state is poisoned".to_string())?;
        state.set_labels(show, hide, quit);
    }
    refresh(&app)
}

/// Update the menu-bar text and the progress rows.
///
/// Only the labels are preserved: this is the command the queue pushes while a
/// run is in flight, so it is the one that decides what "current progress" means.
#[tauri::command]
pub async fn update_tray_progress(
    app: AppHandle,
    progress: TrayProgress,
) -> Result<(), String> {
    {
        let mut state = STATE
            .lock()
            .map_err(|_| "the tray state is poisoned".to_string())?;
        state.set_progress(progress);
    }
    refresh(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalization: what reaches the menu at all ------------------------

    #[test]
    fn rows_are_trimmed_and_blanks_dropped() {
        assert_eq!(
            normalize_rows(["  clip.mp4  ", "", "   ", "Transcribing · 42%"]),
            vec!["clip.mp4", "Transcribing · 42%"]
        );
    }

    #[test]
    fn at_most_three_rows_are_shown() {
        assert_eq!(normalize_rows(["a", "b", "c", "d", "e"]), vec!["a", "b", "c"]);
    }

    /// Truncating by bytes would panic on a Chinese title; the char boundary is
    /// what makes this safe rather than merely correct in English.
    #[test]
    fn a_long_row_is_truncated_by_characters_not_bytes() {
        let rows = normalize_rows(["转录进度".repeat(40)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chars().count(), MAX_ROW_CHARS);
        assert!(rows[0].ends_with('…'));
    }

    #[test]
    fn a_blank_title_clears_the_menu_bar_text() {
        assert_eq!(normalize_title(None), None);
        assert_eq!(normalize_title(Some("   ".to_string())), None);
        assert_eq!(normalize_title(Some(" 42% ".to_string())), Some("42%".to_string()));
    }

    // --- state: the two commands must not clobber each other ----------------

    /// A progress push keeps the localised labels and vice versa; without this a
    /// language change mid-run would leave the menu half in each locale.
    #[test]
    fn labels_and_progress_do_not_overwrite_each_other() {
        let mut state = TrayState::default();
        state.set_labels("显示".into(), "隐藏".into(), "退出".into());
        state.set_progress(TrayProgress {
            title: Some("42%".into()),
            rows: vec!["clip.mp4".into()],
        });

        assert_eq!(state.title.as_deref(), Some("42%"));
        assert_eq!(state.rows, vec!["clip.mp4"]);
        assert_eq!(state.labels.show, "显示");

        state.set_progress(TrayProgress::default());
        assert_eq!(state.title, None);
        assert!(state.rows.is_empty());
        assert_eq!(state.labels.quit, "退出");
    }

    // --- layout: the menu's shape -----------------------------------------

    fn ids(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.id.as_str()).collect()
    }

    fn texts(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.text.as_str()).collect()
    }

    /// Idle: no separator, because there is nothing above it to separate.
    #[test]
    fn an_idle_menu_is_three_actions_and_no_separator() {
        let rows = layout(&TrayState::default());
        assert_eq!(ids(&rows), vec!["show", "hide", "quit"]);
        assert!(rows.iter().all(|row| row.kind == RowKind::Action));
    }

    /// A left click must answer with progress first and actions last.
    #[test]
    fn status_rows_lead_and_the_actions_trail() {
        let mut state = TrayState::default();
        state.set_progress(TrayProgress {
            title: Some("42%".into()),
            rows: vec!["clip.mp4 — Transcribing (3/4)".into(), "2 more queued".into()],
        });

        let rows = layout(&state);
        assert_eq!(
            ids(&rows),
            vec![
                "tray-status-0",
                "tray-status-1",
                "tray-separator",
                "show",
                "hide",
                "quit",
            ]
        );
        assert_eq!(
            texts(&rows),
            vec![
                "clip.mp4 — Transcribing (3/4)",
                "2 more queued",
                "",
                "Show Window",
                "Hide Window",
                "Quit",
            ]
        );
    }

    /// Only the actions are clickable. This is the flag the menu builder passes to
    /// `MenuItem::with_id`, so a progress row can never be mistaken for a command.
    #[test]
    fn only_the_actions_are_enabled() {
        let mut state = TrayState::default();
        state.set_progress(TrayProgress {
            title: None,
            rows: vec!["a".into(), "b".into()],
        });

        let rows = layout(&state);
        let enabled: Vec<bool> = rows.iter().map(Row::enabled).collect();
        assert_eq!(enabled, vec![false, false, false, true, true, true]);
    }

    /// Ids are position-stable and the actions keep their order, so a re-render
    /// replaces rather than accumulates and "Quit is the bottom row" survives.
    #[test]
    fn action_ids_are_stable_and_ordered() {
        let actions: Vec<String> = layout(&TrayState::default())
            .into_iter()
            .filter(|row| row.kind == RowKind::Action)
            .map(|row| row.id)
            .collect();
        assert_eq!(actions, vec!["show", "hide", "quit"]);
    }

    /// A Chinese menu must be Chinese end to end — not English actions under
    /// Chinese status rows.
    #[test]
    fn the_actions_use_the_pushed_labels() {
        let mut state = TrayState::default();
        state.set_labels("显示主窗口".into(), "隐藏窗口".into(), "退出".into());
        state.set_progress(TrayProgress {
            title: None,
            rows: vec!["clip.mp4 — 转录中（3/4）".into()],
        });

        assert_eq!(
            texts(&layout(&state)),
            vec!["clip.mp4 — 转录中（3/4）", "", "显示主窗口", "隐藏窗口", "退出"]
        );
    }

    // --- the frontend handshake -------------------------------------------

    /// The exact payload `src/lib/tray.ts` sends must decode, including the
    /// omitted `title` of the idle case (that is what clears the menu-bar text).
    ///
    /// This pins the wire shape: the frontend is TypeScript with no test runner,
    /// so this is where a renamed field would be caught.
    #[test]
    fn the_frontend_payload_decodes() {
        let progress: TrayProgress = serde_json::from_str(
            r#"{"title":"42%","rows":["clip.mp4 — 转录中（3/4）","队列中还有 2 个"]}"#,
        )
        .expect("the in-flight payload decodes");
        assert_eq!(progress.title.as_deref(), Some("42%"));
        assert_eq!(progress.rows.len(), 2);

        // The idle push is `{}`: no title key at all, and no rows.
        let idle: TrayProgress = serde_json::from_str("{}").expect("the idle payload decodes");
        assert_eq!(idle.title, None);
        assert!(idle.rows.is_empty());

        // An explicit null title is the same as omitting it.
        let cleared: TrayProgress =
            serde_json::from_str(r#"{"title":null,"rows":[]}"#).expect("null decodes");
        assert_eq!(cleared.title, None);
    }

    /// Idle must clear the menu-bar title, not leave the last percentage. The
    /// state transition is the whole point of the idle push, so it is asserted on
    /// the state rather than on a menu that needs the real main thread to build.
    #[test]
    fn an_idle_push_clears_the_menu_bar_text() {
        let mut state = TrayState::default();
        state.set_progress(TrayProgress {
            title: Some("42%".into()),
            rows: vec!["clip.mp4".into()],
        });
        assert_eq!(state.title.as_deref(), Some("42%"));
        assert_eq!(layout(&state).len(), 5, "1 row + separator + 3 actions");

        state.set_progress(serde_json::from_str("{}").unwrap());
        assert_eq!(state.title, None, "no stale percentage in the menu bar");
        assert_eq!(layout(&state).len(), 3, "back to 3 actions, no separator");
    }
}
