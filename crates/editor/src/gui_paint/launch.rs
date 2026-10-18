//! Hidden command-line options for opening Paint mode directly and for automated screenshots:
//!
//!     materialcraft-gui --paint <file.mcpaint|file.spp> [--channel <id>] [--layout 3d|2d|both] [--shot <out.png>]
//!     materialcraft-gui --paint-demo [--channel <id>] [--shot <out.png>]
//!     materialcraft-gui --new-project <mesh.obj|mesh.glb> [--shot <out.png>]
//!     materialcraft-gui --paint <file> --occlusion-check <map.png>
//!     materialcraft-gui --paint <file> --stroke-test [--stroke-helpers] --shot <out.png>
//!
//! `--new-project` makes a project from the mesh file with the first built-in template, as File > New project would.
//! `--channel` takes a channel id (`base_color`, `roughness`, ...) or `material`; `--layout` starts with the 3D view,
//! the 2D (UV) view or both side by side; `--shot` saves the
//! window once every map is on the GPU and then quits. `--occlusion-check` compares the CPU view projector
//! with the GPU's depth buffer once the scene has settled, prints the agreement, saves a map of it and quits
//! (use it without `--shot`). `--stroke-test` paints one stroke on a new paint layer once the document has
//! settled (before `--shot` saves the window); with `--stroke-helpers` it does so with mirror symmetry across X and
//! the lazy mouse on, and adds a straight line in angle steps from the stroke's end.

use super::channel_view::ChannelView;
use super::view_state::ViewLayout;
use materialcraft::paint::Channel;
use std::path::PathBuf;

/// What to open at start-up.
#[derive(Clone, Debug, PartialEq)]
pub enum StartDocument {
    File(PathBuf),
    /// A small document built in code (for testing without files).
    Demo,
    /// A new project from this mesh file with the default template.
    NewProject(PathBuf),
}

#[derive(Clone, Debug, Default)]
pub struct Launch {
    pub open: Option<StartDocument>,
    pub channel: Option<ChannelView>,
    pub layout: Option<ViewLayout>,
    pub shot: Option<PathBuf>,
    pub occlusion_check: Option<PathBuf>,
    pub stroke_test: bool,
    pub stroke_helpers: bool,
}

impl Launch {
    /// Parses the options above; other arguments are ignored.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Launch {
        let mut launch = Launch::default();
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--paint" => launch.open = it.next().map(|p| StartDocument::File(PathBuf::from(p))),
                "--paint-demo" => launch.open = Some(StartDocument::Demo),
                "--new-project" => launch.open = it.next().map(|p| StartDocument::NewProject(PathBuf::from(p))),
                "--shot" => launch.shot = it.next().map(PathBuf::from),
                "--occlusion-check" => launch.occlusion_check = it.next().map(PathBuf::from),
                "--stroke-test" => launch.stroke_test = true,
                "--stroke-helpers" => launch.stroke_helpers = true,
                "--channel" => launch.channel = it.next().and_then(|c| parse_channel(&c)),
                "--layout" => {
                    launch.layout = it.next().and_then(|l| match l.as_str() {
                        "3d" => Some(ViewLayout::Mesh),
                        "2d" => Some(ViewLayout::Uv),
                        "both" => Some(ViewLayout::Both),
                        _ => None,
                    })
                }
                _ => {}
            }
        }
        launch
    }
}

fn parse_channel(id: &str) -> Option<ChannelView> {
    if id == "material" {
        return Some(ChannelView::Material);
    }
    let c = Channel::from_id(id)?;
    ChannelView::ALL.iter().map(|(v, _)| *v).find(|v| *v == ChannelView::Single(c))
}
