//! The Paint screen's window integration: its title and files dropped onto the window.

use eframe::egui;
use std::path::PathBuf;

/// Sends the window title when it changes.
#[derive(Default)]
pub struct WindowTitle {
    sent: String,
}

impl WindowTitle {
    pub fn update(&mut self, ctx: &egui::Context, document: Option<&str>) {
        let title = match document {
            Some(name) => format!("MaterialCraft Paint - {name}"),
            None => "MaterialCraft Paint".to_string(),
        };
        if title != self.sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.sent = title;
        }
    }

    /// Someone else set the title meanwhile (e.g. the node editor); send ours again next time.
    pub fn forget(&mut self) {
        self.sent.clear();
    }
}

/// The first `.mcpaint` or `.spp` file dropped onto the window this frame.
pub fn dropped_document(ctx: &egui::Context) -> Option<PathBuf> {
    let dropped: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.clone())
            .collect()
    });
    dropped.into_iter().find(|p| {
        matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("mcpaint" | "spp")
        )
    })
}
