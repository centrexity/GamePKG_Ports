//! Maintainer-only mirror of the native shader resource/validation contract.
//! Production preparation must still use GamePKG's signed compiler path.
fn main() {
    let mut checked = 0;
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).unwrap();
        let module = if path.ends_with(".wgsl") {
            let source = std::str::from_utf8(&bytes).unwrap();
            naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{path}: {}", e.emit_to_string(source)))
        } else {
            naga::front::spv::parse_u8_slice(&bytes, &naga::front::spv::Options::default())
                .unwrap_or_else(|e| panic!("{path}: {e:?}"))
        };
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{path}: {e:?}"));
        assert_eq!(module.entry_points.len(), 1);
        assert!(matches!(
            module.entry_points[0].stage,
            naga::ShaderStage::Vertex | naga::ShaderStage::Fragment
        ));
        if path.ends_with(".wgsl") {
            let mut varyings = 0;
            for (_, ty) in module.types.iter() {
                if let naga::TypeInner::Struct { members, .. } = &ty.inner {
                    for member in members {
                        let name = member.name.as_deref().unwrap_or("");
                        for (prefix, expected) in [
                            ("diffuse", 0),
                            ("specular", 1),
                            ("t0_", 2),
                            ("t1_", 3),
                            ("t2_", 4),
                            ("t3_", 5),
                            ("fog_", 6),
                        ] {
                            if name.starts_with(prefix) {
                                if let Some(naga::Binding::Location { location, .. }) =
                                    member.binding
                                {
                                    assert_eq!(location, expected, "{path}: varying {name}");
                                    varyings += 1;
                                }
                            }
                        }
                    }
                }
            }
            assert_eq!(
                varyings, 7,
                "all seven raster varyings require the coordinated ABI"
            );
        }
        let mut textures = std::collections::BTreeSet::new();
        let mut samplers = std::collections::BTreeSet::new();
        for (_, v) in module.global_variables.iter() {
            if v.binding.is_none()
                && matches!(
                    v.space,
                    naga::AddressSpace::Private | naga::AddressSpace::Function
                )
            {
                continue;
            }
            let b = v.binding.as_ref().expect("bound resource");
            assert_eq!(b.group, 0);
            match (&v.space, &module.types[v.ty].inner) {
                (naga::AddressSpace::Uniform, naga::TypeInner::Struct { span, .. })
                    if b.binding == 0 =>
                {
                    assert_eq!(*span, 13072)
                }
                (
                    naga::AddressSpace::Handle,
                    naga::TypeInner::Image {
                        dim: naga::ImageDimension::D2 | naga::ImageDimension::Cube,
                        arrayed: false,
                        class:
                            naga::ImageClass::Sampled {
                                kind: naga::ScalarKind::Float,
                                multi: false,
                            },
                    },
                ) if b.binding >= 1 && b.binding <= 7 && b.binding % 2 == 1 => {
                    textures.insert(b.binding);
                }
                (naga::AddressSpace::Handle, naga::TypeInner::Sampler { comparison: false })
                    if b.binding >= 2 && b.binding <= 8 && b.binding % 2 == 0 =>
                {
                    samplers.insert(b.binding - 1);
                }
                _ => panic!("{path}: unsupported resource {b:?}"),
            }
        }
        assert_eq!(textures, samplers, "paired texture/sampler bindings");
        checked += 1;
    }
    assert_eq!(checked, 64, "16 dimensions x two stages x WGSL/SPIR-V");
    println!(
        "PASS: 64 compiled stages pass Naga validation, resource bounds and NVI1 uniform layout"
    );
}
