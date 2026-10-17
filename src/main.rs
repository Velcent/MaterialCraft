// SPDX-License-Identifier: MIT OR Apache-2.0
use materialcraft::bake::{self, BakeSettings};
use materialcraft::export::{self, Format};
use materialcraft::graph::{EXAMPLE, Graph, SPECS};
use materialcraft::render3d::{self, Maps, View};
use materialcraft::{ops, png, tex::Tex};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const HELP: &str = "materialcraft - procedural material authoring (pure Rust)

USAGE:
  materialcraft render <graph.mcg|.mca> <outdir> [--size N | --sizes 256,512] [--depth 8|16|32]
                  [--param \"Label=value\"]... [--variations N] [--preview] [--render3d]\n                  [--format png|jpg|tiff|exr|hdr] [--quality 1-100] [--only basecolor,normal]\n                  [--colorspace srgb|p3|wide|romm|rec2020|linear]
  materialcraft publish <graph.mcg> <out.mca>       embed bitmaps + flatten into a distributable package
  materialcraft params  <graph.mcg|.mca>            list exposed parameters
  materialcraft check   <graph.mcg>
  materialcraft bake    <low.obj> <high.obj> <outdir> [--size N] [--dist D] [--ao-rays N] [--ao-dist D] [--padding N]
  materialcraft testmesh <outdir>                   write a demo low/high-poly OBJ pair
  materialcraft example <outdir>
  materialcraft nodes

The visual editor is a separate binary:  cargo run --release -p materialcraft-editor
";

fn write_png(path: &Path, t: &Tex, depth: u8) -> Result<(), String> {
    materialcraft::safeio::write_file(path, &png::encode(t.w, t.h, &t.px, depth)).map_err(|e| format!("{}: {e}", path.display()))
}

fn load(path: &str) -> Result<Graph, String> {
    // live links are read relative to the file; a link that cannot be read is reported and the graph still loads
    let (g, issues) = Graph::load_file(std::path::Path::new(path)).map_err(|e| format!("{path}: {e}"))?;
    for i in &issues {
        eprintln!("warning: {i}");
    }
    g.validate().map_err(|e| e.to_string())?;
    Ok(g)
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(|s| s.as_str()) {
        Some("nodes") => {
            for s in SPECS.iter().chain(materialcraft::func::FUNC_SPECS).chain(materialcraft::fxmap::FXM_SPECS) {
                println!("{:<18} {:<11} inputs: {:<22} {}", s.kind, materialcraft::graph::category(s.kind), [s.req, s.opt].concat().join(","), s.doc);
            }
            Ok(())
        }
        Some("check") => {
            let g = load(args.get(1).ok_or("usage: materialcraft check <graph.mcg>")?)?;
            println!(
                "OK: {} nodes, {} subgraphs, {} outputs, {}x{}",
                g.nodes.len(),
                g.subgraphs.len(),
                g.outputs.len(),
                g.size,
                if g.is_square() { g.size } else { g.height }
            );
            Ok(())
        }
        Some("params") => {
            let g = load(args.get(1).ok_or("usage: materialcraft params <graph>")?)?;
            for e in &g.exposed {
                println!("\"{}\"  node {} . {}  range {}..{}", e.label, e.node, e.param, e.min, e.max);
            }
            Ok(())
        }
        Some("publish") => {
            let (i, o) = (args.get(1).ok_or("usage: materialcraft publish <in.mcg> <out.mca>")?, args.get(2).ok_or("missing <out.mca>")?);
            let g = load(i)?;
            let text = g.publish().map_err(|e| e.to_string())?;
            materialcraft::safeio::write_file(Path::new(o), text.as_bytes()).map_err(|e| e.to_string())?;
            println!("published {o} ({} bytes, {} exposed parameters)", text.len(), g.exposed.len());
            Ok(())
        }
        Some("example") => {
            let dir = args.get(1).ok_or("usage: materialcraft example <outdir>")?;
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let p = Path::new(dir).join("brick_wall.mcg");
            materialcraft::safeio::write_file(&p, EXAMPLE.as_bytes()).map_err(|e| e.to_string())?;
            println!("wrote {}", p.display());
            Ok(())
        }
        Some("testmesh") => {
            let dir = args.get(1).ok_or("usage: materialcraft testmesh <outdir>")?;
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            fs::write(Path::new(dir).join("low.obj"), bake::uv_sphere_obj(24, 16, 1.0, 0.0)).map_err(|e| e.to_string())?;
            fs::write(Path::new(dir).join("high.obj"), bake::uv_sphere_obj(160, 100, 1.0, 0.04)).map_err(|e| e.to_string())?;
            println!("wrote {dir}/low.obj and {dir}/high.obj");
            Ok(())
        }
        Some("bake") => {
            if args.len() < 4 {
                return Err("usage: materialcraft bake <low.obj> <high.obj> <outdir> [options]".into());
            }
            let mut s = BakeSettings::default();
            let mut i = 4;
            while i < args.len() {
                let v = args.get(i + 1).ok_or_else(|| format!("{} needs a value", args[i]))?;
                match args[i].as_str() {
                    "--size" => s.size = v.parse().map_err(|_| "bad --size")?,
                    "--dist" => s.max_dist = v.parse().map_err(|_| "bad --dist")?,
                    "--ao-rays" => s.ao_rays = v.parse().map_err(|_| "bad --ao-rays")?,
                    "--ao-dist" => s.ao_dist = v.parse().map_err(|_| "bad --ao-dist")?,
                    "--padding" => s.padding = v.parse().map_err(|_| "bad --padding")?,
                    o => return Err(format!("unknown option '{o}'")),
                }
                i += 2;
            }
            let low = bake::parse_obj(&fs::read_to_string(&args[1]).map_err(|e| format!("{}: {e}", args[1]))?)?;
            let high = bake::parse_obj(&fs::read_to_string(&args[2]).map_err(|e| format!("{}: {e}", args[2]))?)?;
            println!("baking {} low tris against {} high tris at {}px ...", low.tris.len(), high.tris.len(), s.size);
            let maps = bake::bake(&low, &high, &s)?;
            fs::create_dir_all(&args[3]).map_err(|e| e.to_string())?;
            for (name, t) in &maps {
                let p = Path::new(&args[3]).join(format!("{name}.png"));
                write_png(&p, t, 8)?;
                println!("wrote {}", p.display());
            }
            Ok(())
        }
        Some("render") => {
            let gp = args.get(1).ok_or("usage: materialcraft render <graph> <outdir> [options]")?;
            let dir = args.get(2).ok_or("missing <outdir>")?;
            let mut g = load(gp)?;
            let (mut sizes, mut depth, mut preview, mut r3d, mut variations) = (vec![g.size], 8u8, false, false, 1u32);
            #[allow(unused_mut, unused_variables)]
            let mut space = String::from("srgb");
            let mut params: BTreeMap<String, f32> = BTreeMap::new();
            let (mut fmt, mut quality, mut only): (Format, u8, Option<Vec<String>>) = (Format::Png, 92, None);
            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--size" => {
                        i += 1;
                        sizes = vec![args.get(i).and_then(|s| s.parse().ok()).ok_or("--size needs a number")?];
                    }
                    "--sizes" => {
                        i += 1;
                        let v = args.get(i).ok_or("--sizes needs a list")?;
                        sizes = v.split(',').map(|s| s.trim().parse::<usize>().map_err(|_| format!("bad size '{s}'"))).collect::<Result<_, _>>()?;
                    }
                    "--depth" => {
                        i += 1;
                        depth = match args.get(i).map(|s| s.as_str()) {
                            Some("8") => 8,
                            Some("16") => 16,
                            Some("32") => 32,
                            _ => return Err("--depth must be 8, 16 or 32".into()),
                        };
                    }
                    "--param" => {
                        i += 1;
                        let kv = args.get(i).ok_or("--param needs Label=value")?;
                        let (k, v) = kv.rsplit_once('=').ok_or("--param must look like \"Label=value\"")?;
                        params.insert(k.to_string(), v.parse().map_err(|_| format!("bad value in '{kv}'"))?);
                    }
                    "--variations" => {
                        i += 1;
                        variations = args.get(i).and_then(|s| s.parse().ok()).filter(|n| (1..=1000).contains(n)).ok_or("--variations needs 1..1000")?;
                    }
                    "--format" => {
                        i += 1;
                        fmt = Format::parse(args.get(i).ok_or("--format needs png, jpg, tiff, exr or hdr")?)
                            .ok_or("--format must be png, jpg, tiff, exr or hdr")?;
                    }
                    "--quality" => {
                        i += 1;
                        quality = args.get(i).and_then(|s| s.parse().ok()).filter(|q| (1..=100).contains(q)).ok_or("--quality needs 1..100")?;
                    }
                    "--only" => {
                        i += 1;
                        only = Some(args.get(i).ok_or("--only needs a comma-separated list")?.split(',').map(|s| s.trim().to_string()).collect());
                    }
                    "--colorspace" => {
                        i += 1;
                        let v = args.get(i).ok_or("--colorspace needs a name")?;
                        #[cfg(any(feature = "cms", feature = "craft"))]
                        {
                            if materialcraft::colorspace::find(v).is_none() {
                                return Err(format!(
                                    "--colorspace must be one of: {}",
                                    materialcraft::colorspace::SPACES.iter().map(|s| s.0).collect::<Vec<_>>().join(", ")
                                ));
                            }
                            space = v.to_string();
                        }
                        #[cfg(not(any(feature = "cms", feature = "craft")))]
                        return Err(format!("--colorspace {v}: this build has no colour management (build with --features cms)"));
                    }
                    "--preview" => preview = true,
                    "--render3d" => r3d = true,
                    o => return Err(format!("unknown option '{o}'")),
                }
                i += 1;
            }
            g.apply_params(&params)?;
            let base_seed = g.seed_offset;
            let multi = sizes.len() > 1;
            for var in 0..variations {
                g.seed_offset = base_seed.wrapping_add(var * 101);
                for &s in &sizes {
                    let out = g.evaluate(Some(s)).map_err(|e| e.to_string())?;
                    let mut d = Path::new(dir).to_path_buf();
                    if variations > 1 {
                        d = d.join(format!("variation_{var:03}"));
                    }
                    if multi {
                        d = d.join(format!("{s}"));
                    }
                    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
                    for (name, t) in &out {
                        if only.as_ref().map(|o| !o.contains(name)).unwrap_or(false) {
                            continue;
                        }
                        let p = d.join(format!("{name}.{}", fmt.ext()));
                        #[cfg(any(feature = "cms", feature = "craft"))]
                        let conv;
                        #[cfg(any(feature = "cms", feature = "craft"))]
                        let t = if materialcraft::colorspace::is_color_output(name) {
                            conv = materialcraft::colorspace::convert(t, &space)?;
                            &conv
                        } else {
                            t
                        };
                        materialcraft::safeio::write_file(&p, &export::encode(fmt, t, if fmt == Format::Jpeg { 8 } else { depth }, quality))
                            .map_err(|e| format!("{}: {e}", p.display()))?;
                        println!("wrote {}", p.display());
                    }
                    let flat = Tex::solid(s, s, [0.5, 0.5, 1.0, 1.0]);
                    let one = Tex::solid(s, s, [1.0; 4]);
                    let zero = Tex::solid(s, s, [0.0, 0.0, 0.0, 1.0]);
                    let half = Tex::solid(s, s, [0.5, 0.5, 0.5, 1.0]);
                    let (b, n, r, m, a) = (
                        out.get("basecolor").unwrap_or(&half),
                        out.get("normal").unwrap_or(&flat),
                        out.get("roughness").unwrap_or(&half),
                        out.get("metallic").unwrap_or(&zero),
                        out.get("ambient_occlusion").unwrap_or(&one),
                    );
                    if preview {
                        let pv = ops::lit_preview(b, n, r, m, a, [0.5, 0.5, 0.7]);
                        write_png(&d.join("preview.png"), &pv, 8)?;
                        println!("wrote {}", d.join("preview.png").display());
                    }
                    if r3d {
                        let maps = Maps { base: b, normal: n, rough: r, metal: m, ao: a, emissive: out.get("emissive"), height: out.get("height") };
                        for (label, id) in render3d::SHAPES.iter() {
                            let name: String = label.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
                            let v = View { shape: *id, ..View::default() };
                            let img = render3d::render(&maps, &v, 512, 512);
                            write_png(&d.join(format!("render_{name}.png")), &img, 8)?;
                            println!("wrote {}", d.join(format!("render_{name}.png")).display());
                        }
                    }
                }
            }
            Ok(())
        }
        _ => {
            print!("{HELP}");
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
