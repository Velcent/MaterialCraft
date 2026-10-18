//! [`ViewState`]: everything the Paint viewport draws with besides the document: the camera pose, light and
//! height display ([`View`]), the display settings ([`Display`]: camera settings, exposure, tone curve,
//! accumulation, texture sharpness) and whether channel views are lit. Converts to and from the view a document
//! is saved with ([`SavedView`]); the camera being navigated is the document's active camera, whose live values
//! are in `view` and `display.camera` (the camera list's rules are [`CameraList`]'s; the operations on it are in
//! [`super::view_cameras`]).

use super::channel_view::ChannelView;
use materialcraft::gpu3d::{Display, Shading};
use materialcraft::paint::camera_list::CameraList;
use materialcraft::paint::saved_view::{CameraValues, SavedCamera, SavedEnvironment, SavedView};
use materialcraft::render3d::{RELIEF_NONE, View};

#[derive(Clone, Debug, PartialEq)]
pub struct ViewState {
    pub view: View,
    pub display: Display,
    /// Channel and mesh-map views lit by the scene (DS-06; unlit by default, ADR 0008 mc2-vq8).
    pub lit_channels: bool,
    /// The document's cameras; the active one's stored values lag behind the live ones until [`ViewState::sync`].
    pub(super) cameras: CameraList,
    /// The environment's kind as the document named it (kept for documents from newer versions).
    environment_kind: String,
}

impl Default for ViewState {
    fn default() -> ViewState {
        // Painter shows height through the normal map; displacement stays opt-in.
        ViewState {
            view: View { relief: RELIEF_NONE, ..View::default() },
            display: Display::default(),
            lit_channels: false,
            cameras: CameraList::default(),
            environment_kind: SavedEnvironment::STUDIO.to_string(),
        }
    }
}

impl ViewState {
    /// The display settings for drawing `mode`: channel views are unlit unless lit channels are on.
    pub fn display_for(&self, mode: ChannelView) -> Display {
        let unlit = mode != ChannelView::Material && !self.lit_channels;
        Display { shading: if unlit { Shading::Unlit } else { Shading::Lit }, ..self.display }
    }

    /// The name of the camera being navigated.
    pub fn camera_name(&self) -> &str {
        self.cameras.active().map_or(SavedCamera::DEFAULT_NAME, |c| c.name.as_str())
    }

    /// The active camera's live values.
    pub(super) fn live_values(&self) -> CameraValues {
        CameraValues { orbit: self.view.orbit(), settings: self.display.camera, aspect: self.cameras.active().and_then(|c| c.aspect) }
    }

    /// Shows `values` as the active camera's live values.
    pub(super) fn load(&mut self, values: CameraValues) {
        self.view.set_orbit(&values.orbit);
        self.display.camera = values.settings;
    }

    /// Stores the live values in the camera list; a locked camera refuses them and its own values come back.
    pub fn sync(&mut self) {
        let live = self.live_values();
        if !self.cameras.store_live(live)
            && let Some(own) = self.cameras.active().map(SavedCamera::values)
        {
            self.load(own);
        }
    }

    /// The camera list with the live values stored.
    pub fn camera_list(&self) -> CameraList {
        let mut list = self.cameras.clone();
        list.store_live(self.live_values());
        list
    }

    /// The view to save with the document.
    pub fn saved(&self) -> SavedView {
        let (cameras, active_camera) = self.camera_list().into_parts();
        let v = &self.view;
        let environment = SavedEnvironment {
            kind: self.environment_kind.clone(),
            light_yaw: v.light_yaw,
            light_pitch: v.light_pitch,
            light_intensity: v.light_intensity,
            light_follows_camera: v.light_follows_camera,
            ambient: v.ambient,
        };
        SavedView { active_camera, cameras, exposure_stops: self.display.exposure_stops, environment }
    }

    /// Takes the document's view: its cameras (the active one becomes the navigated camera), exposure and
    /// environment. Everything else stays as it is.
    pub fn apply_saved(&mut self, saved: &SavedView) {
        self.apply_cameras(saved);
        self.display.exposure_stops = saved.exposure_stops;
        let e = &saved.environment;
        self.environment_kind = e.kind.clone();
        let v = &mut self.view;
        (v.light_yaw, v.light_pitch, v.light_intensity, v.light_follows_camera, v.ambient) =
            (e.light_yaw, e.light_pitch, e.light_intensity, e.light_follows_camera, e.ambient);
    }

    /// Takes only the cameras of `saved` (a freshly imported project keeps the user's exposure and environment).
    pub fn apply_cameras(&mut self, saved: &SavedView) {
        self.cameras = CameraList::new(saved.cameras.clone(), saved.active_camera);
        if let Some(values) = self.cameras.active().map(SavedCamera::values) {
            self.load(values);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::camera::{Orbit, Projection};

    #[test]
    fn the_saved_view_round_trips_and_keeps_other_cameras() {
        let mut s = ViewState::default();
        s.view.yaw = 1.5;
        s.view.ambient = 0.25;
        s.display.exposure_stops = 1.5;
        s.display.camera.projection = Projection::Orthographic;
        let saved = s.saved();
        assert_eq!(saved.cameras.len(), 1);
        assert_eq!(saved.cameras[0].orbit.yaw, 1.5);
        let mut back = ViewState::default();
        back.apply_saved(&saved);
        assert_eq!(back.saved(), saved);
        assert_eq!((&back.view, &back.display), (&s.view, &s.display));
        // a list with a second, active camera: navigating changes only that one
        let mut two = saved.clone();
        two.cameras.push(SavedCamera { name: "Top".into(), orbit: Orbit { pitch: 1.3, ..Orbit::default() }, ..SavedCamera::default() });
        two.active_camera = 1;
        back.apply_saved(&two);
        assert_eq!((back.camera_name(), back.view.pitch), ("Top", 1.3));
        back.view.yaw = -0.5;
        let again = back.saved();
        assert_eq!(again.cameras[0], two.cameras[0]);
        assert_eq!(again.cameras[1].orbit.yaw, -0.5);
        // a fresh import takes only the cameras
        let mut fresh = ViewState::default();
        fresh.display.exposure_stops = -2.0;
        fresh.apply_cameras(&two);
        assert_eq!((fresh.display.exposure_stops, fresh.camera_name()), (-2.0, "Top"));
    }

    #[test]
    fn channel_views_are_unlit_unless_lit_channels_are_on() {
        let mut s = ViewState::default();
        assert_eq!(s.display_for(ChannelView::Material).shading, Shading::Lit);
        let rough = ChannelView::ALL[2].0;
        assert_eq!(s.display_for(rough).shading, Shading::Unlit);
        s.lit_channels = true;
        assert_eq!(s.display_for(rough).shading, Shading::Lit);
    }
}
