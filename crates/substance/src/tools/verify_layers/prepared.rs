//! [`PreparedSet`]: one texture set of a project, ready to verify: its declared channels, resolution and the
//! texture-cache images of the channels being compared, and its texels on the mesh. Imports and composites the
//! layer stack on request.

use super::project_mesh::ProjectMesh;
use crate::decode::document::MaterialRecord;
use crate::model::PainterTextureSet;
use crate::project::CacheEntryRef;
use crate::translate::BakedMaps;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::channel_table::{channel_for_bit, channel_spec};
use crate::translate::layers::cache_score::{Score, score};
use crate::translate::layers::decal::geometry::SetGeometry;
use crate::translate::layers::island_texels::IslandTexels;
use crate::translate::layers::{BlendTable, ImageSource, ImportedStack, LayerImport, ProjectionTable, SetChannels};
use crate::translate::replacements::ProceduralPlan;
use crate::translate::set_layers::defaults_layer;
use materialcraft::generators;
use materialcraft::mesh::SurfaceMap;
use materialcraft::paint::{BakedMap, Channel, Compositor, FillContext, NoFillSources, TextureSet, TiledImage};
use materialcraft::tex::Tex;
use std::sync::OnceLock;

pub struct PreparedSet<'a> {
    pub painter: &'a PainterTextureSet,
    pub channels: SetChannels,
    pub resolution: (u32, u32),
    entries: Vec<&'a CacheEntryRef>,
    cache: Vec<(Channel, TiledImage)>,
    mesh: Option<&'a ProjectMesh>,
    /// The set's texels on the mesh at `resolution`, rasterised on the first score.
    islands: OnceLock<Option<IslandTexels>>,
    /// The set's surface map at `resolution` (first slot), for generators that need UV islands and polygon fills.
    surface: OnceLock<Option<SurfaceMap>>,
    /// The set's mesh parts, for polygon-fill picks.
    geometry: Option<SetGeometry<'a>>,
    /// Which Substance procedurals are replaced by MaterialCraft's generators (none by default).
    pub plan: ProceduralPlan,
    /// The set's baked mesh maps, which the generators read (empty unless loaded).
    pub baked: Vec<BakedMap>,
}

impl<'a> PreparedSet<'a> {
    /// `None` when the set has no texture cache. Without `mesh` scores compare every texel.
    pub fn new(
        painter: &'a PainterTextureSet,
        cache: &'a CacheChannels<'a>,
        mesh: Option<&'a ProjectMesh>,
    ) -> Option<PreparedSet<'a>> {
        let folder = painter.material_uid?.to_string();
        let entries = cache.entries_of(&folder);
        if entries.is_empty() {
            return None;
        }
        let mut specs = Vec::new();
        for decl in &painter.channels {
            if let Some(c) = channel_for_bit(decl.channel.bit())
                && !specs.iter().any(|s: &materialcraft::paint::ChannelSpec| s.channel == c)
            {
                specs.push(channel_spec(c, Some(decl.format)));
            }
        }
        for e in &entries {
            if let Some(c) = channel_for_bit(e.channel_bit).filter(|c| !specs.iter().any(|s| s.channel == *c)) {
                specs.push(channel_spec(c, None));
            }
        }
        let resolution = painter.resolution().map_or((2048, 2048), |[w, h]| (w, h));
        Some(PreparedSet {
            painter,
            channels: SetChannels::new(&specs),
            resolution,
            entries,
            cache: Vec::new(),
            mesh,
            islands: OnceLock::new(),
            surface: OnceLock::new(),
            geometry: mesh.zip(painter.material_uid).map(|(m, uid)| m.geometry(uid)),
            plan: ProceduralPlan::default(),
            baked: Vec::new(),
        })
    }

    pub fn import(&self, table: &BlendTable, images: &dyn ImageSource) -> ImportedStack {
        self.import_with(table, &ProjectionTable::painter(), images)
    }

    /// [`Self::import`] with another projection mapping (for fits).
    pub fn import_with(
        &self,
        table: &BlendTable,
        projections: &ProjectionTable,
        images: &dyn ImageSource,
    ) -> ImportedStack {
        LayerImport {
            table,
            projections,
            images,
            plan: &self.plan,
        }
        .import_on(self.painter, &self.channels, self.resolution, self.geometry.as_ref())
    }

    /// [`Self::import`] of another copy of this set's stack (e.g. with parts swapped), with this set's channels,
    /// resolution and mesh and the procedural choices of `plan`.
    pub fn import_copy(
        &self,
        painter: &PainterTextureSet,
        table: &BlendTable,
        images: &dyn ImageSource,
        plan: &ProceduralPlan,
    ) -> ImportedStack {
        LayerImport {
            table,
            projections: &ProjectionTable::painter(),
            images,
            plan,
        }
        .import_on(painter, &self.channels, self.resolution, self.geometry.as_ref())
    }

    /// Reads the cache image of `channel` (once); false if there is none or it does not decode.
    pub fn load(&mut self, channel: Channel, cache: &CacheChannels) -> bool {
        if self.cache.iter().any(|(c, _)| *c == channel) {
            return true;
        }
        let Some(entry) = self
            .entries
            .iter()
            .find(|e| channel_for_bit(e.channel_bit) == Some(channel))
        else {
            return false;
        };
        match cache.read(entry) {
            Ok(image) => {
                if image.image.size() != self.resolution {
                    self.resolution = image.image.size();
                    self.islands = OnceLock::new();
                    self.surface = OnceLock::new();
                }
                self.cache.push((channel, image.image));
                true
            }
            Err(_) => false,
        }
    }

    /// Our composite of `channel` (imported layers over Painter's defaults) against the loaded cache image.
    pub fn score(&self, stack: &ImportedStack, channel: Channel) -> Option<Score> {
        let cache = self.cache(channel)?;
        let spec = self.channels.spec(channel)?;
        let ours = self.composite(stack, channel);
        Some(score(&ours, cache, spec, self.islands()))
    }

    /// Reads the set's baked mesh maps (for generators); `records` are the document's material records.
    pub fn load_baked(&mut self, baked: &BakedMaps, records: &[MaterialRecord]) {
        let refs = records
            .iter()
            .find(|r| r.name == self.painter.name)
            .map(|r| r.baked.clone())
            .unwrap_or_default();
        self.baked = baked
            .sources(&self.painter.name, &refs)
            .iter()
            .filter_map(|s| baked.read(s).ok().map(|i| i.map))
            .collect();
    }

    /// The loaded cache image of `channel`.
    pub fn cache(&self, channel: Channel) -> Option<&TiledImage> {
        self.cache.iter().find(|(c, _)| *c == channel).map(|(_, i)| i)
    }

    /// The set's texels on the mesh at the compared resolution, if the mesh could be read.
    pub fn islands(&self) -> Option<&IslandTexels> {
        self.islands
            .get_or_init(|| {
                let uid = self.painter.material_uid?;
                self.mesh?.islands(uid, self.resolution)
            })
            .as_ref()
    }

    /// Our composite of `channel`: the imported layers over Painter's defaults, generators reading the baked
    /// maps and the set's surface map.
    pub fn composite(&self, stack: &ImportedStack, channel: Channel) -> Tex {
        let mut set = TextureSet::new("verify", self.painter.name.clone(), self.resolution);
        set.channels = self.channels.specs().to_vec();
        set.layers.push(defaults_layer(&set));
        set.layers.extend(stack.layers.iter().cloned());
        set.baked = self.baked.clone();
        let surface = self.surface.get_or_init(|| {
            let uid = self.painter.material_uid?;
            self.mesh?.surface(uid, self.resolution)
        });
        let fills = match surface {
            Some(s) => {
                FillContext::with_sources(&NoFillSources).with_surface(s, self.mesh.and_then(ProjectMesh::frame))
            }
            None => FillContext::with_sources(&NoFillSources),
        };
        Compositor::with_effects(&set, generators::builtin())
            .with_fills(&fills)
            .composite_tex(channel)
            .0
    }
}
