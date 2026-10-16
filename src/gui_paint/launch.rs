//! Hidden command-line options for opening Paint mode directly and for automated screenshots:
//!
//!     materialcraft-gui --paint <file.mcpaint|file.spp> [--channel <id>] [--shot <out.png>]
//!     materialcraft-gui --paint-demo [--channel <id>] [--shot <out.png>]
//!
//! `--channel` takes a channel id (`base_color`, `roughness`, ...) or `material`; `--shot` saves the
//! window once every map is on the GPU and then quits.

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
    ChannelView::ALL
        .iter()
        .map(|(v, _)| *v)
        .find(|v| *v == ChannelView::Single(c))
}
