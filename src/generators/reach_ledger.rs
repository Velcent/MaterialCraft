//! The reach ledger: every generator, procedural texture and filter registered in [`super::builtin`], with the reach
//! it declares and the test that proves it. A filter or generator can't be written without declaring its reach
//! ([`crate::paint::effect::FilterKernel::reach`] and [`crate::paint::effect::GeneratorSource::stack_reach`] have no
//! default), and a test fails while a registered id is missing here or an entry names an id that is gone: adding
//! an effect means stating how far it reads, proving it, and recording it below. The compositor uses the reach to
//! recompute only what a change can affect (brush strokes, layer edits).

/// What kind of implementation an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Filter,
    Generator,
}

/// One verified effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub id: &'static str,
    pub kind: Kind,
    /// The reach as declared, in words (a filter's radius depends on its parameters and the resolution).
    pub reach: &'static str,
    /// The test that proves it (`module::test`).
    pub proof: &'static str,
    /// When the reach was last verified (after a change to the effect, verify again and update the date).
    pub verified: &'static str,
}

const POINT: &str = "filters::tests_reach::point_filters_read_only_their_texel";
const AREA: &str = "filters::tests_reach::area_filters_never_read_beyond_their_reach";
const NO_STACK: &str = "generators::tests_reach::generators_that_read_no_stack_never_touch_it";
const DATE: &str = "2026-10-18";

const fn point(id: &'static str) -> Entry {
    Entry { id, kind: Kind::Filter, reach: "texel", proof: POINT, verified: DATE }
}

const fn area(id: &'static str, reach: &'static str) -> Entry {
    Entry { id, kind: Kind::Filter, reach, proof: AREA, verified: DATE }
}

const fn no_stack(id: &'static str) -> Entry {
    Entry { id, kind: Kind::Generator, reach: "reads no stack (baked maps, surface and parameters only)", proof: NO_STACK, verified: DATE }
}

/// Every registered effect. Keep it in the order of the source tables.
pub const LEDGER: &[Entry] = &[
    // mask generators (`GENERATORS`)
    no_stack("mask_editor"),
    no_stack("dirt"),
    no_stack("ambient_occlusion"),
    no_stack("metal_edge_wear"),
    no_stack("curvature"),
    no_stack("mask_builder"),
    no_stack("uv_border_distance"),
    Entry {
        id: "height_combination",
        kind: Kind::Generator,
        reach: "the height of its own layer and of the composite below, at the texel only",
        proof: "generators::tests_reach::height_combination_reads_the_heights_at_its_texel",
        verified: DATE,
    },
    // procedural textures (`proc_textures::TEXTURES`)
    no_stack("clouds_1"),
    no_stack("clouds_2"),
    no_stack("clouds_3"),
    no_stack("perlin"),
    no_stack("fractal_sum"),
    no_stack("moisture"),
    no_stack("anisotropic"),
    no_stack("fibers"),
    no_stack("white_noise"),
    no_stack("grunge_01"),
    no_stack("grunge_02"),
    no_stack("grunge_03"),
    no_stack("grunge_04"),
    no_stack("grunge_05"),
    no_stack("grunge_06"),
    no_stack("grunge_07"),
    no_stack("grunge_08"),
    no_stack("grunge_09"),
    no_stack("grunge_10"),
    no_stack("grunge_11"),
    no_stack("grunge_12"),
    no_stack("grunge_13"),
    no_stack("grunge_14"),
    no_stack("grunge_15"),
    no_stack("dirt_soft"),
    no_stack("rust_fine"),
    no_stack("leaks"),
    no_stack("plaster_cracks"),
    no_stack("concrete"),
    no_stack("scratches"),
    no_stack("grunge_spots"),
    no_stack("smudges"),
    no_stack("spots_1"),
    no_stack("spots_2"),
    no_stack("spots_3"),
    no_stack("gaussian_spots_1"),
    no_stack("gaussian_spots_2"),
    no_stack("cells_1"),
    no_stack("cells_2"),
    no_stack("cells_3"),
    no_stack("cells_4"),
    no_stack("crystal_1"),
    no_stack("crystal_2"),
    no_stack("voronoi_fractal"),
    no_stack("stripes"),
    no_stack("checker"),
    no_stack("weave"),
    no_stack("weave_gaps"),
    no_stack("triangle_grid"),
    no_stack("triangles"),
    no_stack("starburst"),
    // filters (`filters::FILTERS`); area radii are twice the kernel's extent (island padding), whole past MAX_RADIUS
    point("hsl"),
    area("blur", "2 x the gaussian half width (or the three box radii for sigma >= 3)"),
    area("sharpen", "2 x the extent of its blur"),
    point("invert"),
    point("height_adjust"),
    area("bevel", "2 x (distance + its blur extent)"),
    area("mask_outline", "2 x (outline width + its blur extent)"),
    area("slope_blur", "2 x (intensity in texels + 2)"),
    area("warp", "2 x (intensity in texels + 2)"),
    point("finish_rough"),
    point("clamp"),
    point("histogram_scan"),
    point("histogram_shift"),
    point("contrast_luminosity"),
];
