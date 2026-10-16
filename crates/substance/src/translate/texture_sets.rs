//! [`SetBinding`]: which mesh material slot, `document.bin` material and texture-cache folder make up each
//! texture set.
//!
//! The link is Painter's material uid (Verified on all 164 corpus projects): the mesh stores it as `$mat.uid`
//! per material, `document.bin` as `DataMaterial.sceneMaterialUID`, and the cache uses it as folder name
//! (`/texture-cache/<uid>/…`). Names cannot be used: texture sets can be renamed in Painter, so the mesh
//! name (`Material.001`) and the set name (`CERAMIC`) often differ. Several mesh slots can share one uid
//! (Painter's collapsed materials); they become one texture set. Documents also keep materials of meshes
//! that were since replaced; those have no slot and are skipped.

use crate::decode::assbin::MaterialSummary;
use crate::decode::document::MaterialRecord;
use std::collections::BTreeSet;

/// One texture set to build.
#[derive(Debug, Clone, PartialEq)]
pub struct SetSource {
    /// Unique id within the document (the set name, made unique if needed).
    pub id: String,
    /// The document's record; a stand-in named after the mesh material when the document has none.
    pub record: MaterialRecord,
    /// True when `record` came from `document.bin`.
    pub in_document: bool,
    /// First mesh material slot painted by this set.
    pub slot: usize,
    /// Further mesh slots Painter collapsed into this set.
    pub merged_slots: Vec<usize>,
    /// Folder name under `/texture-cache`, if the set has rendered channels.
    pub cache_set: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SetBinding {
    pub sets: Vec<SetSource>,
    /// Document materials no mesh slot uses (left over from replaced meshes).
    pub unused: Vec<MaterialRecord>,
}

impl SetBinding {
    pub fn bind(mesh: &[MaterialSummary], records: &[MaterialRecord], cache_sets: &BTreeSet<String>) -> SetBinding {
        let mut binding = SetBinding::default();
        let mut used = vec![false; records.len()];
        for (slot, material) in mesh.iter().enumerate() {
            let found = find_record(material, records);
            if let Some(i) = found {
                if used[i] {
                    if let Some(set) = binding.sets.iter_mut().find(|s| s.record == records[i]) {
                        set.merged_slots.push(slot);
                        continue;
                    }
                }
                used[i] = true;
            }
            let (record, in_document) = match found {
                Some(i) => (records[i].clone(), true),
                None => (stand_in(material), false),
            };
            let cache_set = cache_folder(&record, material, mesh.len(), cache_sets);
            let id = unique_id(&record.name, &binding.sets);
            binding.sets.push(SetSource {
                id,
                record,
                in_document,
                slot,
                merged_slots: Vec::new(),
                cache_set,
            });
        }
        binding.unused = records
            .iter()
            .zip(&used)
            .filter(|(_, u)| !**u)
            .map(|(r, _)| r.clone())
            .collect();
        binding
    }
}

/// By uid; by name only for meshes without uids.
fn find_record(material: &MaterialSummary, records: &[MaterialRecord]) -> Option<usize> {
    match material.uid {
        Some(uid) => records.iter().position(|r| r.scene_uid == Some(uid)),
        None => records.iter().position(|r| r.name == material.name),
    }
}

fn stand_in(material: &MaterialSummary) -> MaterialRecord {
    MaterialRecord {
        name: material.name.clone(),
        scene_uid: material.uid,
        ..MaterialRecord::default()
    }
}

/// The cache folder of the set's uid; a lone cache folder of a single-slot mesh is taken as is.
fn cache_folder(
    record: &MaterialRecord,
    material: &MaterialSummary,
    slots: usize,
    cache: &BTreeSet<String>,
) -> Option<String> {
    let uid = record.scene_uid.or(material.uid).map(|u| u.to_string());
    match uid {
        Some(uid) if cache.contains(&uid) => Some(uid),
        _ if slots == 1 && cache.len() == 1 => cache.iter().next().cloned(),
        _ => None,
    }
}

fn unique_id(name: &str, sets: &[SetSource]) -> String {
    let base = if name.is_empty() { "set" } else { name };
    let taken = |id: &str| sets.iter().any(|s| s.id == id);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|id| !taken(id))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(name: &str, uid: Option<u64>) -> MaterialSummary {
        MaterialSummary { name: name.into(), uid }
    }

    fn record(name: &str, uid: u64) -> MaterialRecord {
        MaterialRecord {
            name: name.into(),
            scene_uid: Some(uid),
            ..MaterialRecord::default()
        }
    }

    #[test]
    fn binds_by_uid_merges_collapsed_slots_and_skips_stale_records() {
        let mesh = [
            mesh("Material.001", Some(7)),
            mesh("Material.002", Some(9)),
            mesh("Material.003", Some(7)),
        ];
        let records = [record("Old", 1), record("Ceramic", 7), record("Wood", 9)];
        let cache: BTreeSet<String> = ["7".to_string()].into();
        let b = SetBinding::bind(&mesh, &records, &cache);
        assert_eq!(b.sets.len(), 2);
        assert_eq!((b.sets[0].id.as_str(), b.sets[0].slot), ("Ceramic", 0));
        assert_eq!(b.sets[0].merged_slots, [2]);
        assert_eq!(b.sets[0].cache_set.as_deref(), Some("7"));
        assert_eq!(b.sets[1].cache_set, None);
        assert_eq!(b.unused.len(), 1);
    }

    #[test]
    fn falls_back_to_names_and_stand_ins() {
        let mesh = [mesh("A", None), mesh("A", Some(3))];
        let records = [record("A", 5)];
        let b = SetBinding::bind(&mesh, &records, &BTreeSet::new());
        assert!(b.sets[0].in_document);
        assert!(!b.sets[1].in_document);
        assert_eq!(b.sets[1].id, "A 2");
    }

    #[test]
    fn single_slot_takes_the_only_cache_folder() {
        let cache: BTreeSet<String> = ["42".to_string()].into();
        let b = SetBinding::bind(&[mesh("M", None)], &[], &cache);
        assert_eq!(b.sets[0].cache_set.as_deref(), Some("42"));
    }
}
