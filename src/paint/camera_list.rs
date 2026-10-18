//! [`CameraList`]: the document's cameras and which one is active, with the rules of the camera list
//! (requirements CM-01 … CM-08): the first camera is the default camera, the only one navigation moves; navigating
//! while another camera is active hands its view to the default camera first (CM-06), so imported cameras are never
//! moved by accident; locked cameras refuse edits; imported cameras can be restored.
//!
//! The viewport keeps the active camera's live values itself and hands them back with [`CameraList::store_live`]
//! before anything reads the list.

use super::saved_camera::{CameraOrigin, CameraValues, SavedCamera};

#[derive(Debug, Clone, PartialEq)]
pub struct CameraList {
    cameras: Vec<SavedCamera>,
    active: usize,
}

impl Default for CameraList {
    fn default() -> CameraList {
        CameraList { cameras: vec![SavedCamera::default()], active: 0 }
    }
}

impl CameraList {
    /// The list of `cameras` with `active` active. An empty list gets a default camera, an index past the end
    /// means the first camera; the default camera is never locked (navigation moves it).
    pub fn new(mut cameras: Vec<SavedCamera>, active: usize) -> CameraList {
        if cameras.is_empty() {
            cameras.push(SavedCamera::default());
        }
        if let Some(first) = cameras.first_mut() {
            first.locked = false;
        }
        let active = if active < cameras.len() { active } else { 0 };
        CameraList { cameras, active }
    }

    pub fn cameras(&self) -> &[SavedCamera] {
        &self.cameras
    }

    pub fn get(&self, index: usize) -> Option<&SavedCamera> {
        self.cameras.get(index)
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> Option<&SavedCamera> {
        self.cameras.get(self.active)
    }

    /// Stores the viewport's live values in the active camera. A locked camera refuses them: false, and the viewport
    /// shows the camera's own values again.
    pub fn store_live(&mut self, values: CameraValues) -> bool {
        self.cameras.get_mut(self.active).is_some_and(|c| c.values() == values || c.set_values(values))
    }

    /// Makes camera `index` active; its values to show, or `None` for an index past the end.
    pub fn select(&mut self, index: usize) -> Option<CameraValues> {
        let values = self.cameras.get(index)?.values();
        self.active = index;
        Some(values)
    }

    /// The camera after the active one, wrapping around (the shortcut that cycles cameras, CM-02).
    pub fn next_index(&self) -> usize {
        (self.active + 1) % self.cameras.len().max(1)
    }

    /// Before navigating (CM-06): when another camera than the default is active, the default camera takes over its
    /// values and becomes active. True when it switched; the view shown stays the same.
    pub fn take_over(&mut self) -> bool {
        if self.active == 0 {
            return false;
        }
        let Some(values) = self.active().map(SavedCamera::values) else { return false };
        if let Some(default) = self.cameras.first_mut() {
            default.locked = false;
            default.set_values(values);
        }
        self.active = 0;
        true
    }

    /// Sets camera `index` to `values` (the current view; refused while locked). True when it changed.
    pub fn set_from_view(&mut self, index: usize, values: CameraValues) -> bool {
        self.cameras.get_mut(index).is_some_and(|c| c.values() != values && c.set_values(values))
    }

    /// Puts imported camera `index` back to its imported values (CM-07). True when it changed.
    pub fn restore(&mut self, index: usize) -> bool {
        self.cameras.get_mut(index).is_some_and(SavedCamera::restore)
    }

    /// Locks or unlocks camera `index` (the default camera cannot be locked). True when it changed.
    pub fn set_locked(&mut self, index: usize, locked: bool) -> bool {
        match self.cameras.get_mut(index) {
            Some(c) if index > 0 && c.locked != locked => {
                c.locked = locked;
                true
            }
            _ => false,
        }
    }

    /// Renames camera `index` (blank names are refused). True when it changed.
    pub fn rename(&mut self, index: usize, name: &str) -> bool {
        let name = name.trim();
        match self.cameras.get_mut(index) {
            Some(c) if !name.is_empty() && c.name != name => {
                c.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    /// Adds a camera of our own with `values`; its index.
    pub fn add(&mut self, name: &str, values: CameraValues) -> usize {
        self.cameras.push(SavedCamera::own(name, values));
        self.cameras.len() - 1
    }

    /// Removes camera `index` (not the default camera, not a locked one). The active camera stays active; removing
    /// it activates the default camera. True when removed.
    pub fn remove(&mut self, index: usize) -> bool {
        if index == 0 || self.cameras.get(index).is_none_or(|c| c.locked) {
            return false;
        }
        self.cameras.remove(index);
        self.active = match self.active {
            a if a == index => 0,
            a if a > index => a - 1,
            a => a,
        };
        true
    }

    /// Adds imported cameras; one with the name and source of a camera already in the list replaces it (an import run
    /// again, CM-05), unless that one is locked. Returns how many were added or replaced.
    pub fn add_imported(&mut self, cameras: impl IntoIterator<Item = SavedCamera>) -> usize {
        let mut count = 0;
        for camera in cameras {
            let source = source_of(&camera);
            let same = self.cameras.iter().position(|c| c.name == camera.name && source.is_some() && source_of(c) == source);
            match same {
                Some(i) if i > 0 => {
                    if let Some(c) = self.cameras.get_mut(i).filter(|c| !c.locked) {
                        *c = camera;
                        count += 1;
                    }
                }
                _ => {
                    self.cameras.push(camera);
                    count += 1;
                }
            }
        }
        count
    }

    /// The list and the active index, as the document stores them.
    pub fn into_parts(self) -> (Vec<SavedCamera>, usize) {
        (self.cameras, self.active)
    }
}

fn source_of(c: &SavedCamera) -> Option<&str> {
    match &c.origin {
        CameraOrigin::Imported { source, .. } => Some(source),
        CameraOrigin::Own => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Orbit, Projection};

    fn values(yaw: f32) -> CameraValues {
        CameraValues { orbit: Orbit { yaw, ..Orbit::default() }, ..CameraValues::default() }
    }

    fn list() -> CameraList {
        let mut l = CameraList::default();
        l.add_imported([SavedCamera::imported("Shot", "gltf", values(1.0)), SavedCamera::imported("Top", "gltf", values(2.0))]);
        l
    }

    #[test]
    fn navigation_hands_an_imported_view_to_the_default_camera() {
        let mut l = list();
        assert_eq!(l.select(1), Some(values(1.0)));
        assert!(l.take_over());
        assert_eq!(l.active_index(), 0);
        assert_eq!(l.cameras()[0].values(), values(1.0));
        assert_eq!(l.cameras()[0].name, SavedCamera::DEFAULT_NAME);
        assert!(!l.cameras()[1].is_modified(), "the imported camera did not move");
        assert!(!l.take_over(), "the default camera navigates itself");
        assert_eq!(l.select(7), None);
        assert_eq!(l.next_index(), 1);
    }

    #[test]
    fn edits_mark_imported_cameras_modified_and_restore_puts_them_back() {
        let mut l = list();
        l.select(2);
        let mut edited = values(2.0);
        edited.settings.projection = Projection::Orthographic;
        assert!(l.store_live(edited));
        assert!(l.cameras()[2].is_modified());
        assert!(l.restore(2));
        assert!(!l.cameras()[2].is_modified() && !l.restore(2));
        assert!(l.set_from_view(1, values(0.25)));
        assert!(l.cameras()[1].is_modified());
        // own cameras have nothing to restore
        let own = l.add("Mine", values(0.5));
        assert!(l.set_from_view(own, values(0.6)) && !l.cameras()[own].is_modified() && !l.restore(own));
    }

    #[test]
    fn locked_cameras_refuse_edits_and_removal() {
        let mut l = list();
        assert!(!l.set_locked(0, true), "the default camera is never locked");
        assert!(l.set_locked(1, true));
        assert!(!l.set_from_view(1, values(3.0)) && !l.remove(1));
        l.select(1);
        assert!(!l.store_live(values(3.0)), "a locked camera keeps its values");
        assert!(l.store_live(values(1.0)), "its own values are fine");
        if let Some(c) = l.cameras.get_mut(1) {
            c.orbit.yaw = 9.0;
        }
        assert!(!l.restore(1));
        // a locked camera is not replaced by a new import either
        assert_eq!(l.add_imported([SavedCamera::imported("Shot", "gltf", values(5.0))]), 0);
        assert!(l.rename(1, " Hero ") && l.cameras()[1].name == "Hero" && !l.rename(1, "  "));
    }

    #[test]
    fn importing_again_replaces_by_name_and_source_and_removal_keeps_the_active_camera() {
        let mut l = list();
        assert_eq!(l.add_imported([SavedCamera::imported("Shot", "gltf", values(4.0)), SavedCamera::imported("Shot", "painter-scene", values(5.0))]), 2);
        assert_eq!(l.cameras().len(), 4);
        assert_eq!(l.cameras()[1].values(), values(4.0));
        l.select(3);
        assert!(l.remove(1) && l.active_index() == 2 && l.active().is_some_and(|c| c.name == "Shot"));
        assert!(l.remove(2) && l.active_index() == 0);
        assert!(!l.remove(0));
        let empty = CameraList::new(Vec::new(), 5);
        assert_eq!((empty.cameras().len(), empty.active_index()), (1, 0));
    }
}
