//! Importing cameras from a glTF file into the open document's camera list (CM-04, CM-05: camera import can run again
//! later). The file's cameras are read in the document mesh's coordinates, so they fit documents whose mesh came from
//! the same scene; importing again replaces the cameras of the same name.

use super::PaintMode;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::mesh::gltf::read_cameras;
use materialcraft::paint::camera_import::imported_cameras;

/// Source id of cameras from glTF files (one of `CameraOrigin::SOURCES`).
const GLTF: &str = "gltf";

impl PaintMode {
    /// Asks for a `.glb` file and adds its cameras.
    pub(super) fn import_cameras(&mut self) {
        let Some(doc) = &self.document else { return };
        let Some(path) = rfd::FileDialog::new().add_filter("glTF binary", &["glb"]).pick_file() else { return };
        let read = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| read_cameras(&bytes).map_err(|e| e.to_string()));
        let cameras = match read {
            Ok(c) => imported_cameras(&doc.doc().mesh, &c, GLTF),
            Err(e) => {
                self.diag.error(Area::Project, format!("{}: {e}", path.display()));
                return;
            }
        };
        let found = cameras.len();
        let (state, _) = self.viewport.settings_mut();
        let taken = state.import_cameras(cameras);
        let message = match (found, taken) {
            (0, _) => format!("{} has no usable cameras.", path.display()),
            (f, t) if t < f => format!("Imported {t} of {f} cameras from {}; locked cameras of the same name were kept.", path.display()),
            (f, _) => format!("Imported {f} camera{} from {}.", if f == 1 { "" } else { "s" }, path.display()),
        };
        self.diag.info(Area::Project, message);
    }
}
