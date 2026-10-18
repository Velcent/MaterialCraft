use super::*;
use crate::history::{CoalesceKey, ShaderHistory};
use crate::mesh::LengthUnit;

fn decl(params: &str) -> Result<ShaderDecl, DeclError> {
    ShaderDecl::parse(&format!(r#"{{"format":"mcshader","format_version":1,"id":"t","vertex":"t.vert","fragment":"t.frag","params":[{params}]}}"#))
}

#[test]
fn the_builtin_declaration_parses_and_round_trips() {
    let f = ShaderFile::builtin();
    assert_eq!(f.id(), DEFAULT_SHADER);
    assert!(f.decl.params.len() >= 4, "the built-in declaration failed to parse");
    assert_eq!(f.decl.groups(), vec!["Surface", "Subsurface"]);
    let again = ShaderDecl::parse(&f.decl.to_json().to_string_pretty()).unwrap();
    assert_eq!(again, f.decl);
    // every declared uniform is used by the program
    for p in &f.decl.params {
        assert!(f.fragment_src.contains(&p.uniform), "{} is not in pbr.frag", p.uniform);
    }
}

#[test]
fn every_parameter_type_parses() {
    let d = decl(
        r#"{"id":"f","uniform":"uF","type":"float","min":0,"max":2,"default":1,"step":0.5,"group":"G","label":"Float"},
           {"id":"i","uniform":"uI","type":"int","min":-1,"max":3,"default":0},
           {"id":"b","uniform":"uB","type":"bool","default":true},
           {"id":"c","uniform":"uC","type":"color","default":[0.25,0.5,1]},
           {"id":"e","uniform":"uE","type":"choice","options":["one","two"],"default":"two"}"#,
    )
    .unwrap();
    assert_eq!(d.name, "t", "the name defaults to the id");
    assert_eq!(d.param("f").unwrap().kind, ParamKind::Float { min: 0.0, max: 2.0, default: 1.0, step: Some(0.5) });
    assert_eq!((d.param("f").unwrap().label.as_str(), d.param("i").unwrap().label.as_str()), ("Float", "i"));
    assert_eq!(d.param("i").unwrap().kind, ParamKind::Int { min: -1, max: 3, default: 0 });
    assert_eq!(d.param("b").unwrap().default_value(), ParamValue::Bool(true));
    assert_eq!(d.param("c").unwrap().default_value(), ParamValue::Color([0.25, 0.5, 1.0]));
    assert_eq!(d.param("e").unwrap().default_value(), ParamValue::Int(1));
    assert_eq!(d.groups(), vec!["G", ""]);
    assert_eq!(ShaderDecl::parse(&d.to_json().to_string()).unwrap(), d);
}

#[test]
fn bad_declarations_are_refused_with_their_place() {
    let cases = [
        (r#"{"id":"a","uniform":"uA","type":"float","min":1,"max":0,"default":0}"#, "params[0].max"),
        (r#"{"id":"a","uniform":"uA","type":"float","min":0,"max":1,"default":2}"#, "params[0].default"),
        (r#"{"id":"a","uniform":"uA","type":"float","min":0,"max":1}"#, "params[0].default"),
        (r#"{"id":"a","uniform":"uA","type":"vector"}"#, "params[0].type"),
        (r#"{"id":"a","uniform":"1x","type":"bool"}"#, "params[0].uniform"),
        (r#"{"id":"a","uniform":"gl_Foo","type":"bool"}"#, "params[0].uniform"),
        (r#"{"id":"a","uniform":"uBase","type":"bool"}"#, "params[0].uniform"),
        (r#"{"id":"a","uniform":"uA","type":"color","default":[1,2,0]}"#, "params[0].default"),
        (r#"{"id":"a","uniform":"uA","type":"choice","options":[]}"#, "params[0].options"),
        (r#"{"id":"a","uniform":"uA","type":"choice","options":["x","x"]}"#, "params[0].options"),
        (r#"{"id":"a","uniform":"uA","type":"choice","options":["x"],"default":"y"}"#, "params[0].default"),
        (r#"{"id":"a","uniform":"uA","type":"bool"},{"id":"a","uniform":"uB","type":"bool"}"#, "params[1].id"),
        (r#"{"id":"a","uniform":"uA","type":"bool"},{"id":"b","uniform":"uA","type":"bool"}"#, "params[1].uniform"),
        (r#"{"uniform":"uA","type":"bool"}"#, "params[0].id"),
    ];
    for (params, at) in cases {
        let e = decl(params).unwrap_err();
        assert_eq!(e.at, at, "{params}: {e}");
    }
    assert_eq!(ShaderDecl::parse("[]").unwrap_err().at, "");
    assert_eq!(ShaderDecl::parse(r#"{"format":"other"}"#).unwrap_err().at, "format");
    assert_eq!(ShaderDecl::parse(r#"{"format":"mcshader","format_version":2}"#).unwrap_err().at, "format_version");
    assert_eq!(ShaderDecl::parse(r#"{"format":"mcshader","format_version":1,"id":"x","vertex":"v"}"#).unwrap_err().at, "fragment");
    assert!(ShaderDecl::parse("{").is_err());
}

#[test]
fn shader_files_load_from_a_folder() {
    let dir = std::env::temp_dir().join(format!("mc-shading-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let json = r#"{"format":"mcshader","format_version":1,"id":"my.toon","vertex":"toon.vert","fragment":"toon.frag",
                   "params":[{"id":"bands","uniform":"uBands","type":"int","min":1,"max":8,"default":3}]}"#;
    std::fs::write(dir.join("toon.mcshader"), json).unwrap();
    std::fs::write(dir.join("toon.vert"), "// v").unwrap();
    std::fs::write(dir.join("toon.frag"), "// f").unwrap();
    std::fs::write(dir.join("broken.mcshader"), "{").unwrap();
    std::fs::write(dir.join("builtin.mcshader"), json.replace("my.toon", DEFAULT_SHADER)).unwrap();
    let mut lib = ShaderLibrary::default();
    let errors = lib.load_folder(&dir);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(lib.files().len(), 2, "the built-in shader can't be replaced");
    let toon = lib.find("my.toon").unwrap();
    assert_eq!((toon.vertex_src.as_str(), toon.fragment_src.as_str()), ("// v", "// f"));
    assert_eq!(lib.find_or_default("missing").id(), DEFAULT_SHADER);
    assert!(lib.load_folder(std::path::Path::new("/no/such/folder")).is_empty());
}

#[test]
fn instances_resolve_values_and_detect_outdated_files() {
    let file = ShaderFile::builtin();
    let mut inst = ShaderInstance::new("a", "A", &file);
    let uniforms = inst.uniforms(&file);
    assert_eq!(uniforms.len(), file.decl.params.len());
    assert!(uniforms.iter().any(|u| u.uniform == "uAoStrength" && u.value == ParamValue::Float(0.85)));
    inst.set_value("ao_strength", Some(ParamValue::Float(7.0)));
    inst.set_value("gone", Some(ParamValue::Bool(true)));
    let ao = file.decl.param("ao_strength").unwrap();
    assert_eq!(inst.value(ao), ParamValue::Float(1.0), "clamped on use");
    assert!(!inst.is_outdated(&file));
    let mut changed = file.decl.clone();
    changed.revision += 1;
    let newer = ShaderFile::new(changed, file.vertex_src.clone(), String::new(), None);
    assert!(inst.is_outdated(&newer));
    let up = inst.updated_to(&newer);
    assert!(!up.is_outdated(&newer));
    assert_eq!(up.values, vec![("ao_strength".to_string(), ParamValue::Float(1.0))], "unknown values dropped, the rest made valid");
    let reset = inst.with_defaults(&file);
    assert_eq!((reset.id.as_str(), reset.name.as_str(), reset.values.len()), ("a", "A", 0));
}

fn settings_with_instance() -> (ShaderSettings, ShaderFile) {
    let file = ShaderFile::builtin();
    let mut s = ShaderSettings::default();
    ShaderEdit::Insert { at: 0, instance: Box::new(ShaderInstance::new("shader", "Shared", &file)), sets: vec!["s1".into(), "s2".into()] }
        .apply(&mut s)
        .unwrap();
    (s, file)
}

#[test]
fn texture_sets_share_instances() {
    let (mut s, file) = settings_with_instance();
    assert_eq!(s.instance_for("s1").map(|i| i.id.as_str()), Some("shader"));
    assert_eq!(s.sets_using("shader"), vec!["s1", "s2"]);
    assert!(s.instance_for("s3").is_none(), "unassigned sets use the built-in defaults");
    assert_eq!(s.new_instance_id(), "shader_2");
    assert_eq!(ShaderEdit::Assign { set: "s3".into(), instance: Some("nope".into()) }.apply(&mut s), Err(ShaderEditError::UnknownInstance("nope".into())));
    let dup = ShaderEdit::Insert { at: 9, instance: Box::new(ShaderInstance::new("shader", "X", &file)), sets: vec![] };
    assert_eq!(dup.apply(&mut s), Err(ShaderEditError::DuplicateId("shader".into())));
}

#[test]
fn the_local_undo_stack_reverts_every_edit() {
    let (mut s, file) = settings_with_instance();
    let start = s.clone();
    let mut h = ShaderHistory::default();
    let drag = Some(CoalesceKey(7));
    for v in [0.2, 0.4, 0.6] {
        h.apply(&mut s, "AO", drag, ShaderEdit::SetValue { instance: "shader".into(), param: "ao_strength".into(), value: Some(ParamValue::Float(v)) })
            .unwrap();
    }
    h.close();
    let disp = Displacement { enabled: true, scale: 0.1, unit: DisplacementUnit::Centimetres, ..Displacement::default() };
    h.apply(&mut s, "Displacement", None, ShaderEdit::SetDisplacement { instance: "shader".into(), value: disp }).unwrap();
    let tess = Tessellation { mode: TessellationMode::Uniform { levels: 3 }, smooth: true };
    h.apply(&mut s, "Tessellation", None, ShaderEdit::SetTessellation { instance: "shader".into(), value: tess }).unwrap();
    h.apply(&mut s, "Rename", None, ShaderEdit::Rename { instance: "shader".into(), name: "Skin".into() }).unwrap();
    h.apply(&mut s, "Unit", None, ShaderEdit::SetMeshUnit(Some(LengthUnit::Centimetre))).unwrap();
    let second = ShaderInstance::new("shader_2", "Other", &file);
    h.apply(&mut s, "New", None, ShaderEdit::Insert { at: 1, instance: Box::new(second), sets: vec!["s2".into()] }).unwrap();
    let reset = s.instance("shader").unwrap().with_defaults(&file);
    h.apply(&mut s, "Restore defaults", None, ShaderEdit::Replace { instance: Box::new(reset) }).unwrap();
    h.apply(&mut s, "Delete", None, ShaderEdit::Remove { instance: "shader".into(), restore: vec![] }).unwrap();
    assert_eq!(h.len(), 8, "one drag is one step");
    assert!(s.instance_for("s1").is_none());
    assert_eq!(s.instance_for("s2").map(|i| i.id.as_str()), Some("shader_2"));
    let end = s.clone();
    // step back once: the deleted instance and its assignment return
    h.undo(&mut s).unwrap();
    assert_eq!(s.sets_using("shader"), vec!["s1"]);
    h.jump_to(&mut s, 1).unwrap();
    let ao = s.instance("shader").unwrap().stored("ao_strength");
    assert_eq!(ao, Some(ParamValue::Float(0.6)));
    h.jump_to(&mut s, 0).unwrap();
    assert_eq!(s, start, "undoing everything restores the start");
    h.jump_to(&mut s, usize::MAX).unwrap();
    assert_eq!(s, end, "redoing everything restores the end");
}
