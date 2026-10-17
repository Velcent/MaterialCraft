//! The stroke record's [`Camera`] (plain matrices, stored per stroke) and the camera module's
//! [`crate::camera::Camera`] (the same matrices plus the viewport, with the screen ↔ world maths). A stored stroke
//! camera has no viewport: its samples are in normalised device coordinates, so any viewport of the right aspect
//! replays it.

use super::record::Camera;
use crate::camera;

impl From<camera::Camera> for Camera {
    fn from(c: camera::Camera) -> Camera {
        Camera { view: c.view, projection: c.projection }
    }
}

impl Camera {
    /// The camera with the screen ↔ world maths, on a viewport of `viewport` units.
    pub fn on_viewport(&self, viewport: [f32; 2]) -> camera::Camera {
        camera::Camera::new(self.view, self.projection, viewport)
    }

    /// The camera on a viewport with the aspect ratio its projection was made for (`height` units high), so stored
    /// sample positions keep their shape on screen. A projection that does not say its aspect gets a square one.
    pub fn on_viewport_of_height(&self, height: f32) -> camera::Camera {
        let aspect = camera::Lens::aspect_of(&self.projection).unwrap_or(1.0);
        self.on_viewport([height * aspect, height])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Lens;

    #[test]
    fn stroke_cameras_convert_to_and_from_the_camera_type() {
        let lens = Lens::Perspective { fov_y: 0.8, near: 0.05, far: 40.0 };
        let cam = camera::Camera::looking_at([0.5, 1.0, 3.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [1280.0, 720.0]);
        let stored = Camera::from(cam);
        assert_eq!(stored.view, cam.view);
        assert_eq!(stored.projection, cam.projection);
        assert_eq!(stored.on_viewport([1280.0, 720.0]), cam);
        // the viewport comes back from the projection's aspect
        let back = stored.on_viewport_of_height(720.0);
        assert!((back.viewport[0] - 1280.0).abs() < 0.01 && back.viewport[1] == 720.0);
        // a stored sample (NDC, y up) lands where the world point projects
        let world = [0.2, -0.1, 0.3];
        let p = cam.project(world).unwrap();
        let ndc = cam.screen_to_ndc(p.screen);
        let on_screen = back.ndc_to_screen(ndc);
        assert!((on_screen[0] - p.screen[0]).abs() < 0.01 && (on_screen[1] - p.screen[1]).abs() < 0.01);
        // the default stroke camera is the identity and stays one
        let id = Camera::default().on_viewport([2.0, 2.0]);
        assert_eq!(Camera::from(id), Camera::default());
    }
}
