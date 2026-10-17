//! The workspace gate, headless: layouts and shortcuts survive a restart through the settings
//! file; the viewport can't be undocked.

use super::*;
use crate::gui_settings::SavedLayout;
use crate::gui_workspace::PanelKind;
use crate::gui_workspace::shortcuts::chord::Chord;
use craft_settings::Location;
use craft_ui::docking::{Action, Floating, Layout, Node, Placement, Zone};
use materialcraft::diag::NullSink;
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("materialcraft-shell-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn shell_at(dir: &std::path::Path) -> Shell {
    let (settings, problems) = UserSettings::open(Location::Dir(dir.to_path_buf()));
    assert!(problems.is_empty(), "{problems:?}");
    Shell::new(settings, Arc::new(NullSink))
}

#[test]
fn the_layout_and_shortcuts_survive_a_restart() {
    let dir = temp_dir("restart");
    let before: Layout<PaintPanel> = {
        let mut shell = shell_at(&dir);
        let ws = &mut shell.workspace;
        ws.close(PaintPanel::ImportReport).expect("close");
        ws.apply(Action::Move { panel: PaintPanel::TextureSets, anchor: PaintPanel::Display, placement: Placement::Tab { before: None } }).expect("move");
        ws.toggle_popover(PaintPanel::Shortcuts, [400.0, 80.0, 320.0, 400.0]).expect("popover");
        assert!(shell.apply(PanelEvent::Shortcut(PageEdit::Set("paint.tool.eraser", vec![Chord::parse("E").expect("chord")])), true).is_none());
        let layout = shell.workspace.layout().clone();
        shell.flush();
        layout
    };
    let mut shell = shell_at(&dir);
    assert_eq!(shell.workspace.layout(), &before);
    assert_eq!(shell.shortcuts.label("paint.tool.eraser"), "E");
    // The closed panel comes back where it was.
    shell.workspace.open(PaintPanel::ImportReport).expect("reopen");
    assert!(shell.workspace.is_open(PaintPanel::ImportReport));
    let text = std::fs::read_to_string(dir.join("config.toml")).expect("config");
    assert!(text.contains("[workspace") && text.contains("paint.tool.eraser"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_saved_layout_can_be_switched_to_after_a_restart() {
    let dir = temp_dir("named");
    {
        let mut shell = shell_at(&dir);
        shell.workspace.close(PaintPanel::Display).expect("close");
        shell.apply(PanelEvent::SaveLayout("Mine".into()), true);
        shell.apply(PanelEvent::SwitchLayout("Baking".into()), true);
        assert!(shell.workspace.is_open(PaintPanel::Display));
        shell.flush();
    }
    let mut shell = shell_at(&dir);
    assert_eq!(shell.layout_name, "Baking");
    assert!(shell.layout_names().contains(&("Mine".to_string(), true)));
    shell.apply(PanelEvent::SwitchLayout("Mine".into()), true);
    assert!(!shell.workspace.is_open(PaintPanel::Display));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_viewport_cannot_be_undocked_moved_closed_or_covered() {
    let dir = temp_dir("viewport");
    let mut shell = shell_at(&dir);
    let ws = &mut shell.workspace;
    let v = PaintPanel::Viewport;
    let before = ws.layout().clone();
    assert!(ws.apply(Action::Float { panel: v, rect: [10.0, 10.0, 300.0, 300.0] }).is_err());
    assert!(ws.apply(Action::Move { panel: v, anchor: PaintPanel::Display, placement: Placement::Split(Zone::Left) }).is_err());
    assert!(ws.apply(Action::Close { panel: v }).is_err());
    assert!(ws.close(v).is_err());
    assert!(ws.toggle(v).is_err());
    assert!(ws.toggle_popover(v, [10.0, 10.0, 300.0, 300.0]).is_err());
    assert!(ws.apply(Action::Move { panel: PaintPanel::Display, anchor: v, placement: Placement::Tab { before: None } }).is_err(), "nothing tabs into it");
    assert_eq!(ws.layout(), &before);
    // Panels may still split beside it.
    ws.apply(Action::Move { panel: PaintPanel::Display, anchor: v, placement: Placement::Split(Zone::Bottom) }).expect("split beside");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stored_layout_with_a_floating_or_missing_viewport_is_refused() {
    let id = |p: PaintPanel| p.id().to_string();
    let floating = SavedLayout {
        layout: Layout {
            root: Some(Node::Tabs { panels: vec![id(PaintPanel::Display)], active: 0 }),
            floating: vec![Floating { panels: vec![id(PaintPanel::Viewport)], active: 0, rect: [0.0, 0.0, 400.0, 300.0] }],
        },
        ..Default::default()
    };
    assert!(Workspace::<PaintPanel>::from_saved(dock_id(), &floating).is_err());
    let missing = SavedLayout {
        layout: Layout { root: Some(Node::Tabs { panels: vec![id(PaintPanel::Display)], active: 0 }), floating: Vec::new() },
        ..Default::default()
    };
    assert!(Workspace::<PaintPanel>::from_saved(dock_id(), &missing).is_err());
    let unknown = SavedLayout {
        layout: Layout { root: Some(Node::Tabs { panels: vec!["viewport".into(), "plugins".into()], active: 0 }), floating: Vec::new() },
        ..Default::default()
    };
    assert!(Workspace::<PaintPanel>::from_saved(dock_id(), &unknown).is_err());
    // On start-up such a file falls back to a working layout.
    let dir = temp_dir("fallback");
    {
        let (mut settings, _) = UserSettings::open(Location::Dir(dir.clone()));
        settings.update(|s| s.workspace.current = Some(floating.clone()));
        settings.flush().expect("flush");
    }
    let shell = shell_at(&dir);
    assert!(shell.workspace.is_open(PaintPanel::Viewport));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_popover_becomes_an_ordinary_panel_once_moved() {
    let dir = temp_dir("popover");
    let mut shell = shell_at(&dir);
    let ws = &mut shell.workspace;
    ws.toggle_popover(PaintPanel::Interface, [400.0, 80.0, 300.0, 200.0]).expect("open");
    assert_eq!(ws.popover(), Some(PaintPanel::Interface));
    ws.toggle_popover(PaintPanel::Interface, [400.0, 80.0, 300.0, 200.0]).expect("hide");
    assert!(!ws.is_open(PaintPanel::Interface) && ws.popover().is_none());
    ws.toggle_popover(PaintPanel::Interface, [400.0, 80.0, 300.0, 200.0]).expect("open again");
    ws.apply(Action::MoveFloating { panel: PaintPanel::Interface, rect: [200.0, 200.0, 300.0, 200.0] }).expect("drag away");
    assert_eq!(ws.popover(), None);
    assert!(ws.is_open(PaintPanel::Interface));
    let _ = std::fs::remove_dir_all(&dir);
}
