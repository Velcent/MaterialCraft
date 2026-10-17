//! Every [`Subject`] the tool knows. Adding one (e.g. a procedural generator against a texture-cache
//! channel) means a new module here plus one line in [`all`].

pub mod baked;

use crate::subject::Subject;

/// All subjects, in listing order.
pub fn all() -> Vec<Box<dyn Subject>> {
    vec![
        Box::new(baked::AmbientOcclusion),
        Box::new(baked::Curvature),
        Box::new(baked::TangentNormal),
        Box::new(baked::WorldNormal),
        Box::new(baked::Position),
        Box::new(baked::Thickness),
        Box::new(baked::IdMap),
    ]
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
        assert_eq!(ids.len(), 7);
    }
}
