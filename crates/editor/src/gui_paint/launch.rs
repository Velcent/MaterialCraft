//! Hidden command-line options for opening Paint mode directly and for automated screenshots:
//!
//!     materialcraft-gui --paint <file.mcpaint|file.spp> [--channel <id>] [--shot <out.png>]
//!     materialcraft-gui --paint-demo [--channel <id>] [--shot <out.png>]
//!     materialcraft-gui --paint <file> --occlusion-check <map.png>
//!
//! `--channel` takes a channel id (`base_color`, `roughness`, ...) or `material`; `--shot` saves the
//! window once every map is on the GPU and then quits. `--occlusion-check` compares the CPU view projector
//! with the GPU's depth buffer once the scene has settled, prints the agreement, saves a map of it and quits
//! (use it without `--shot`).

use super::channel_view::ChannelView;
use materialcraft::paint::Channel;
use std::path::PathBuf;

/// What to open at start-up.
#[derive(Clone, Debug, PartialEq)]
pub enum StartDocument {
    File(PathBuf),
    /// A small document built in code (for testing without files).
    Demo,
}

#[derive(Clone, Debug, Default)]
pub struct Launch {
    pub open: Option<StartDocument>,
    pub channel: Option<ChannelView>,
    pub shot: Option<PathBuf>,
    pub occlusion_check: Option<PathBuf>,
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
                "--shot" => launch.shot = it.next().map(PathBuf::from),
                "--occlusion-check" => launch.occlusion_check = it.next().map(PathBuf::from),
                "--channel" => launch.channel = it.next().and_then(|c| parse_channel(&c)),
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
