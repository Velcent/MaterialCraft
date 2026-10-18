use super::*;
use crate::paint::{BlendMode, Channel, Fill};

#[test]
fn kinds_ids_and_builtins() {
    let fill = ContentEffect::new(Vec::new(), EffectKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.2; 4])])))
        .with_blend(Channel::Roughness, BlendMode::Normal, 1.0)
        .with_blend(Channel::Roughness, BlendMode::Multiply, 0.5);
    assert_eq!(fill.blend.len(), 1);
    assert_eq!(fill.blend_for(Channel::Roughness).map(|b| b.mode), Some(BlendMode::Multiply));
    assert_eq!(fill.kind.id(), "fill");
    assert!(fill.kind.is_builtin());
    assert_eq!(fill.channel_lists(), vec![vec![Channel::Roughness], vec![Channel::Roughness]]);
    let generator: MaskEffect = Effect::new(GreyBlend::default(), EffectKind::Generator(Procedural::new("dirt")));
    assert_eq!(generator.kind.id(), "generator");
    assert!(!generator.kind.is_builtin());
    let unknown: MaskEffect = Effect::new(GreyBlend::default(), EffectKind::Unknown(UnknownEffect { kind: "warp3d".into(), data: crate::json::Value::Null }));
    assert_eq!(unknown.kind.id(), "warp3d");
    for m in PickMode::ALL {
        assert_eq!(PickMode::from_id(m.id()), Some(m));
    }
    let mut p = Procedural::new("blur");
    p.params.push(("radius".into(), "2".into()));
    assert_eq!(p.param("radius"), Some("2"));
    assert_eq!(Levels::default().gamma, [1.0; 3]);
}

#[test]
fn registry_finds_and_replaces() {
    struct Flat(f32);
    impl GeneratorSource for Flat {
        fn stack_reach(&self, _: &EvalRequest) -> Option<Reach> {
            None
        }
        fn generate(&self, _: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
            out.fill([self.0, self.0, self.0, 1.0]);
            Ok(())
        }
    }
    let mut r = EffectRegistry::new();
    assert!(r.generator("flat").is_none());
    r.add_generator("flat", Box::new(Flat(0.1)));
    r.add_generator("flat", Box::new(Flat(0.2)));
    assert!(r.generator("flat").is_some());
    assert!(r.filter("flat").is_none());
    assert!(format!("{r:?}").contains("flat"));
}
