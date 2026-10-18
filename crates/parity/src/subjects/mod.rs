//! Every [`Subject`] the tool knows. Adding one means a new module here plus one line in [`all`].

pub mod baked;
pub mod brush;
pub mod filters;
pub mod generators;
pub mod textures;

use crate::subject::Subject;

/// All subjects, in listing order.
pub fn all() -> Vec<Box<dyn Subject>> {
    let baked: Vec<Box<dyn Subject>> = vec![
        Box::new(baked::AmbientOcclusion),
        Box::new(baked::Curvature),
        Box::new(baked::TangentNormal),
        Box::new(baked::WorldNormal),
        Box::new(baked::Position),
        Box::new(baked::Thickness),
        Box::new(baked::IdMap),
    ];
    baked
        .into_iter()
        .chain(generators::all())
        .chain(textures::all())
        .chain(filters::all())
        .chain(std::iter::once(Box::new(brush::Replay) as Box<dyn Subject>))
        .collect()
}

/// The subjects a command-line selector names: `all`, an exact id (`baked.ao`), or an id prefix (`baked`).
pub fn select(selector: &str) -> Vec<Box<dyn Subject>> {
    all()
        .into_iter()
        .filter(|s| {
            selector == "all"
                || s.id() == selector
                || s.id().strip_prefix(selector).is_some_and(|rest| rest.starts_with('.'))
                || s.id().rsplit('.').next() == Some(selector)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_by_id_prefix_and_short_name() {
        assert_eq!(select("all").len(), all().len());
        assert_eq!(select("baked").len(), 7);
        assert_eq!(select("baked.ao")[0].id(), "baked.ao");
        assert_eq!(select("normal")[0].id(), "baked.normal");
        assert!(select("bake").is_empty());
        let mut ids: Vec<_> = all().iter().map(|s| s.id()).collect();
        ids.dedup();
        assert_eq!(ids.len(), all().len());
        assert!(select("generator").len() >= 7);
        assert_eq!(select("generator.dirt")[0].generator(), Some("dirt"));
        assert_eq!(select("texture").len(), materialcraft::proc_textures::TEXTURES.len());
        assert_eq!(select("texture.clouds_2")[0].generator(), Some("clouds_2"));
        assert_eq!(select("filter").len(), materialcraft::filters::FILTERS.len());
        assert_eq!(select("filter.blur")[0].generator(), Some("blur"));
        assert_eq!(select("brush").len(), 1);
        assert_eq!(select("brush.replay")[0].generator(), Some("stroke_replay"));
    }
}
