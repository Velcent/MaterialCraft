//! Full geometry of the scene as a [`Mesh`]; see [`super::part`] for the conventions.

use super::RECORD;
use super::faces::polygon_starts;
use super::material::MaterialSummary;
use super::mesh_block::MeshBlock;
use super::node::{MeshInstance, mesh_instances};
use super::part::build_part;
use super::scene::{SceneBlocks, scene_bytes};
use super::transform::Transform;
use crate::decode::DecodeError;
use materialcraft::mesh::{FaceGroups, Mesh};

/// Upper bound on vertices after instancing (the corpus maximum is far below a million),
/// so a node tree repeating a big mesh cannot exhaust memory.
const MAX_PLACED_VERTICES: usize = 1 << 26;

/// Decodes all vertex data. Use [`super::AssbinScene`] when only counts and names are needed.
pub struct AssbinGeometry;

impl AssbinGeometry {
    /// One part per mesh placement in the node tree (a mesh placed by two nodes gives two parts,
    /// named `mesh [node]`); meshes no node places are kept once, untransformed.
    /// The materials are the texture set names.
    pub fn parse(data: &[u8]) -> Result<Mesh, DecodeError> {
        Self::parse_with_faces(data).map(|(mesh, _)| mesh)
    }

    /// [`Self::parse`] plus each part's face groups (Painter's source polygons, from the per-face flag), kept beside
    /// the mesh for polygon-fill picks.
    pub fn parse_with_faces(data: &[u8]) -> Result<(Mesh, Vec<FaceGroups>), DecodeError> {
        let scene = scene_bytes(data)?;
        let blocks = SceneBlocks::split(&scene)?;
        let meshes = blocks
            .meshes
            .iter()
            .map(|b| MeshBlock::parse(b))
            .collect::<Result<Vec<_>, _>>()?;
        let materials = blocks
            .materials
            .iter()
            .map(|b| MaterialSummary::parse(b).map(|m| m.name))
            .collect::<Result<Vec<_>, _>>()?;
        let instances = placements(mesh_instances(blocks.root_node)?, meshes.len())?;
        let mut uses = vec![0usize; meshes.len()];
        instances.iter().for_each(|i| uses[i.mesh as usize] += 1);
        let placed: usize = instances.iter().map(|i| meshes[i.mesh as usize].vertex_count()).sum();
        if placed > MAX_PLACED_VERTICES {
            return Err(DecodeError::invalid(
                RECORD,
                format!("{placed} placed vertices is too many"),
            ));
        }
        let mut parts = Vec::with_capacity(instances.len());
        let mut faces = Vec::with_capacity(instances.len());
        for instance in &instances {
            let mesh = &meshes[instance.mesh as usize];
            if mesh.material_index as usize >= materials.len() {
                return Err(DecodeError::invalid(
                    RECORD,
                    format!("mesh '{}' uses missing material {}", mesh.name, mesh.material_index),
                ));
            }
            let name = match uses[instance.mesh as usize] > 1 {
                true => format!("{} [{}]", mesh.name, instance.node),
                false => mesh.name.clone(),
            };
            let mut part = build_part(mesh, &instance.world, name)?;
            let groups = FaceGroups::from_polygon_starts(polygon_starts(mesh.faces, mesh.face_count)?);
            // the document keeps them with the part (PT-47)
            part.faces = groups.clone();
            parts.push(part);
            faces.push(groups);
        }
        Ok((Mesh { materials, parts }, faces))
    }
}

/// Checks the node tree's mesh indices and adds an identity placement for every unplaced mesh.
fn placements(mut instances: Vec<MeshInstance>, mesh_count: usize) -> Result<Vec<MeshInstance>, DecodeError> {
    if let Some(bad) = instances.iter().find(|i| i.mesh as usize >= mesh_count) {
        return Err(DecodeError::invalid(
            RECORD,
            format!("node '{}' places missing mesh {}", bad.node, bad.mesh),
        ));
    }
    let mut placed = vec![false; mesh_count];
    instances.iter().for_each(|i| placed[i.mesh as usize] = true);
    for (mesh, _) in placed.iter().enumerate().filter(|(_, p)| !**p) {
        instances.push(MeshInstance {
            mesh: mesh as u32,
            node: String::new(),
            world: Transform::IDENTITY,
        });
    }
    Ok(instances)
}
