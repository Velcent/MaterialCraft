//! The camera list's operations on the viewport's [`ViewState`] (CM-02 … CM-08): switching cameras, the hand-over
//! to the default camera before navigating, set from view, restore, lock, rename, add, remove and import. Each stores
//! the live values first and shows the active camera's values afterwards; the rules are
//! [`materialcraft::paint::camera_list::CameraList`]'s.

use super::view_state::ViewState;
use materialcraft::paint::saved_view::SavedCamera;

impl ViewState {
    /// True when the active camera refuses edits.
    pub fn active_locked(&self) -> bool {
        self.cameras.active().is_some_and(|c| c.locked)
    }

    /// Shows camera `index`.
    pub fn select_camera(&mut self, index: usize) {
        self.sync();
        if let Some(values) = self.cameras.select(index) {
            self.load(values);
        }
    }

    /// Shows the next camera, wrapping around.
    pub fn next_camera(&mut self) {
        self.select_camera(self.cameras.next_index());
    }

    /// Before a navigation move (CM-06): another camera than the default hands its view to the default camera,
    /// which becomes active; the picture stays the same. True when it switched.
    pub fn before_navigation(&mut self) -> bool {
        self.sync();
        self.cameras.take_over()
    }

    /// Sets camera `index` to the current view. True when it changed.
    pub fn set_camera_from_view(&mut self, index: usize) -> bool {
        self.sync();
        let live = self.live_values();
        self.cameras.set_from_view(index, live)
    }

    /// Puts imported camera `index` back to its imported values. True when it changed.
    pub fn restore_camera(&mut self, index: usize) -> bool {
        self.sync();
        let changed = self.cameras.restore(index);
        self.show_active();
        changed
    }

    pub fn lock_camera(&mut self, index: usize, locked: bool) -> bool {
        self.sync();
        self.cameras.set_locked(index, locked)
    }

    pub fn rename_camera(&mut self, index: usize, name: &str) -> bool {
        self.cameras.rename(index, name)
    }

    /// Adds the current view as a new camera named `name`; its index.
    pub fn add_camera_from_view(&mut self, name: &str) -> usize {
        self.sync();
        let live = self.live_values();
        self.cameras.add(name, live)
    }

    pub fn remove_camera(&mut self, index: usize) -> bool {
        self.sync();
        let removed = self.cameras.remove(index);
        self.show_active();
        removed
    }

    /// Adds imported cameras (re-imports replace cameras of the same name and source). How many were taken.
    pub fn import_cameras(&mut self, cameras: Vec<SavedCamera>) -> usize {
        self.sync();
        let count = self.cameras.add_imported(cameras);
        self.show_active();
        count
    }

    fn show_active(&mut self) {
        if let Some(values) = self.cameras.active().map(SavedCamera::values) {
            self.load(values);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::camera::{Orbit, Projection};
    use materialcraft::paint::saved_view::{CameraValues, SavedView};

    fn state() -> ViewState {
        let mut s = ViewState::default();
        let shot = CameraValues { orbit: Orbit { yaw: 2.0, roll: 0.1, ..Orbit::default() }, aspect: Some(1.5), ..CameraValues::default() };
        s.apply_saved(&SavedView { cameras: vec![SavedCamera::default(), SavedCamera::imported("Shot", "gltf", shot)], ..SavedView::default() });
        s
    }

    #[test]
    fn navigating_an_imported_camera_hands_its_view_to_the_default_camera() {
        let mut s = state();
        s.select_camera(1);
        assert_eq!((s.camera_name(), s.view.yaw, s.view.roll), ("Shot", 2.0, 0.1));
        assert!(s.before_navigation());
        assert_eq!((s.camera_name(), s.view.yaw, s.view.roll), (SavedCamera::DEFAULT_NAME, 2.0, 0.1), "the picture stays");
        s.view.yaw = 2.5;
        let saved = s.saved();
        assert_eq!(saved.cameras[0].orbit.yaw, 2.5);
        assert!(!saved.cameras[1].is_modified(), "the imported camera never moved");
        assert!(!s.before_navigation());
        s.next_camera();
        assert_eq!(s.camera_name(), "Shot");
    }

    #[test]
    fn edits_lock_restore_and_set_from_view() {
        let mut s = state();
        s.select_camera(1);
        s.display.camera.projection = Projection::Orthographic;
        s.sync();
        assert!(s.camera_list().cameras()[1].is_modified());
        assert!(s.restore_camera(1));
        assert_eq!(s.display.camera.projection, Projection::Perspective, "the restored values show");
        assert!(s.lock_camera(1, true) && s.active_locked());
        s.display.camera.fov_y = 1.0;
        s.sync();
        assert_ne!(s.display.camera.fov_y, 1.0, "a locked camera's edit is undone");
        s.select_camera(0);
        s.view.yaw = -1.0;
        assert!(!s.set_camera_from_view(1), "locked");
        assert!(s.lock_camera(1, false) && s.set_camera_from_view(1));
        assert_eq!(s.camera_list().cameras()[1].orbit.yaw, -1.0);
        let own = s.add_camera_from_view("Mine");
        assert!(s.rename_camera(own, "Close-up") && s.camera_list().cameras()[own].name == "Close-up");
        s.select_camera(own);
        assert!(s.remove_camera(own) && s.camera_name() == SavedCamera::DEFAULT_NAME);
        assert_eq!(s.import_cameras(vec![SavedCamera::imported("Shot", "gltf", CameraValues::default())]), 1);
        assert_eq!(s.camera_list().cameras().len(), 2);
    }
}
