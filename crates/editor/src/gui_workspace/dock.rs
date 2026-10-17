//! [`Workspace`]: a screen's docking layout on craft-ui's model and renderer, with the rules the
//! model leaves to the app: the canvas stays docked, closed panels remember where they were,
//! and panel-strip popovers (WS-01…05, UI-03, UI-06).

use craft_ui::docking::{Action, DockArea, DockContent, DockError, DockStyle, Layout, Location, Node, PanelLimits, Placement, Zone};
use eframe::egui::{Id, Ui};

use super::convert::{map_layout, map_location};
use super::panel::PanelKind;
use crate::gui_settings::SavedLayout;

/// The docking state of one screen.
pub struct Workspace<P: PanelKind> {
    area: Id,
    layout: Layout<P>,
    /// Where each closed panel was.
    hidden: Vec<(P, Location<P>)>,
    /// The panel shown as a popover from the panel strip, until the user moves or docks it.
    popover: Option<P>,
    changed: bool,
}

impl<P: PanelKind> Workspace<P> {
    /// A workspace showing `layout`; refused if the layout is invalid or a canvas isn't docked.
    pub fn new(area: Id, layout: Layout<P>) -> Result<Workspace<P>, String> {
        check(&layout)?;
        Ok(Workspace { area, layout, hidden: Vec::new(), popover: None, changed: false })
    }

    /// Only the canvas panels, tabbed together: the fallback when nothing else loads.
    pub fn minimal(area: Id) -> Workspace<P> {
        let canvas: Vec<P> = P::ALL.iter().copied().filter(|p| p.is_canvas()).collect();
        let root = (!canvas.is_empty()).then_some(Node::Tabs { panels: canvas, active: 0 });
        Workspace { area, layout: Layout { root, floating: Vec::new() }, hidden: Vec::new(), popover: None, changed: false }
    }

    /// The workspace stored in `saved`; refused if it names unknown panels or breaks the rules.
    pub fn from_saved(area: Id, saved: &SavedLayout) -> Result<Workspace<P>, String> {
        let layout = map_layout(&saved.layout, &|s: &String| P::from_id(s)).ok_or("the layout names a panel this version doesn't have")?;
        let mut ws = Workspace::new(area, layout)?;
        ws.hidden = saved
            .hidden
            .iter()
            .filter_map(|(id, loc)| Some((P::from_id(id)?, map_location(loc, &|s: &String| P::from_id(s))?)))
            .filter(|(p, _)| !ws.layout.contains(p))
            .collect();
        Ok(ws)
    }

    /// The workspace in storable form.
    pub fn to_saved(&self) -> SavedLayout {
        let id = |p: &P| Some(p.id().to_owned());
        SavedLayout {
            layout: map_layout(&self.layout, &id).unwrap_or_default(),
            hidden: self.hidden.iter().filter_map(|(p, loc)| Some((p.id().to_owned(), map_location(loc, &id)?))).collect(),
        }
    }

    #[cfg(test)]
    pub fn layout(&self) -> &Layout<P> {
        &self.layout
    }

    pub fn is_open(&self, panel: P) -> bool {
        self.layout.contains(&panel)
    }

    /// The panel currently shown as a popover, if any.
    #[cfg(test)]
    pub fn popover(&self) -> Option<P> {
        self.popover
    }

    /// True once after the layout changed (to save it).
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Applies one docking action under the panels' permissions and the canvas rule.
    pub fn apply(&mut self, action: Action<P>) -> Result<(), DockError> {
        if let Action::Close { panel } = action {
            return self.close(panel);
        }
        let leaves_popover = match &action {
            Action::Move { panel, .. } | Action::MoveFloating { panel, .. } | Action::Float { panel, .. } => self.popover == Some(*panel),
            _ => false,
        };
        let mut next = self.layout.clone();
        next.apply_with_permissions(action, |p| p.permissions())?;
        check(&next).map_err(|_| DockError::NotAllowed)?;
        self.commit(next);
        if leaves_popover {
            // Dragged away from its button: now an ordinary floating, dockable panel (UI-03).
            self.popover = None;
        }
        Ok(())
    }

    /// Closes `panel`, remembering where it was. The canvas can't be closed.
    pub fn close(&mut self, panel: P) -> Result<(), DockError> {
        if panel.is_canvas() {
            return Err(DockError::NotAllowed);
        }
        let loc = self.layout.location(&panel)?;
        let mut next = self.layout.clone();
        next.apply_with_permissions(Action::Close { panel }, |p| p.permissions())?;
        check(&next).map_err(|_| DockError::NotAllowed)?;
        self.commit(next);
        self.hidden.retain(|(p, _)| *p != panel);
        self.hidden.push((panel, loc));
        if self.popover == Some(panel) {
            self.popover = None;
        }
        Ok(())
    }

    /// Shows `panel`: where it was before it was closed, else beside the canvas, else wherever
    /// the model puts it. An open panel is brought to the front.
    pub fn open(&mut self, panel: P) -> Result<(), DockError> {
        if self.is_open(panel) {
            return self.apply(Action::Activate { panel });
        }
        if let Some(i) = self.hidden.iter().position(|(p, _)| *p == panel) {
            let mut next = self.layout.clone();
            let restored = self.hidden.get(i).is_some_and(|(_, loc)| next.restore(panel, loc).is_ok());
            if restored && check(&next).is_ok() {
                self.commit(next);
                self.hidden.remove(i);
                return Ok(());
            }
        }
        let canvas = P::ALL.iter().copied().find(|p| p.is_canvas() && self.is_open(*p));
        let first = match canvas {
            Some(anchor) => self.apply(Action::OpenAt { panel, anchor, placement: Placement::Split(Zone::Right) }),
            None => Err(DockError::MissingTarget),
        };
        first.or_else(|_| self.apply(Action::Open { panel, anchor: None }))?;
        self.hidden.retain(|(p, _)| *p != panel);
        Ok(())
    }

    pub fn toggle(&mut self, panel: P) -> Result<(), DockError> {
        if self.is_open(panel) { self.close(panel) } else { self.open(panel) }
    }

    /// The panel strip's click (UI-03): a closed panel opens as a popover floating at `rect`
    /// (`[x, y, w, h]`); a second click, or a click on an open panel's button, hides it.
    pub fn toggle_popover(&mut self, panel: P, rect: [f32; 4]) -> Result<(), DockError> {
        if self.is_open(panel) {
            return self.close(panel);
        }
        // The model opens a panel next to an anchor; it floats straight away.
        let anchor = P::ALL.iter().copied().find(|p| p.is_canvas() && self.is_open(*p)).ok_or(DockError::MissingTarget)?;
        let mut next = self.layout.clone();
        next.apply_with_permissions(Action::OpenAt { panel, anchor, placement: Placement::Split(Zone::Right) }, |p| p.permissions())?;
        next.apply_with_permissions(Action::Float { panel, rect }, |p| p.permissions())?;
        check(&next).map_err(|_| DockError::NotAllowed)?;
        self.commit(next);
        self.popover = Some(panel);
        Ok(())
    }

    /// Draws the docked and floating panels through `content` and applies what the user did.
    /// Returns a message when a requested change was refused.
    pub fn show(&mut self, ui: &mut Ui, style: &DockStyle, content: &mut impl DockContent<P>) -> Option<String> {
        let out = DockArea::new(self.area).show_customized(
            ui,
            &self.layout,
            style,
            |p| p.title().to_owned(),
            |p| p.permissions(),
            |_| PanelLimits::default(),
            content,
        );
        // `out.error` is about drawing (a window too small this frame), not a user request.
        let mut error = None;
        for action in out.actions {
            if let Err(e) = self.apply(action) {
                error = Some(e.to_string());
            }
        }
        error
    }

    fn commit(&mut self, next: Layout<P>) {
        if next != self.layout {
            self.layout = next;
            self.changed = true;
        }
    }
}

/// The app's rules on top of the model's validation: every canvas panel is docked in the main
/// tree (not floating, not closed).
pub fn check<P: PanelKind>(layout: &Layout<P>) -> Result<(), String> {
    layout.validate().map_err(|e| e.to_string())?;
    let docked = Layout { root: layout.root.clone(), floating: Vec::new() };
    match P::ALL.iter().find(|p| p.is_canvas() && !docked.contains(p)) {
        Some(p) => Err(format!("the {} must stay docked", p.title())),
        None => Ok(()),
    }
}
