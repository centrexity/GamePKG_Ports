//! Maintainer-only hardware vectors. Direct GPU access belongs only to this
//! test harness, never the port's game objects or packaged runtime.
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::mpsc,
};
use takedawn_nvi1::{Block, CombinerRegisters, Program, Texture, VertexMode};
use wgpu::util::DeviceExt;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    witness: wgpu::RenderPipeline,
    fragment: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    textures: Vec<wgpu::TextureView>,
    sampler: wgpu::Sampler,
}
impl Gpu {
    fn new(vertex: &str, fragment: &str, software_diagnostic: bool) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("hardware Vulkan adapter");
        if !software_diagnostic {
            assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu,
                "hardware qualification unavailable; --software-diagnostic is a separate nonqualifying diagnostic");
        }
        eprintln!(
            "Synthetic NVI1 execution vectors on {}",
            adapter.get_info().name
        );
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .checked();
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(13072),
            },
            count: None,
        }];
        for i in 0..4 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 1 + i * 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: 2 + i * 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let vs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("candidate NVI1 VS"),
            source: wgpu::ShaderSource::Wgsl(vertex.into()),
        });
        let fs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("candidate NVI1 FS"),
            source: wgpu::ShaderSource::Wgsl(fragment.into()),
        });
        // Observe oT0 without a clamp/UNORM conversion to test vertex math.
        let observe=device.create_shader_module(wgpu::ShaderModuleDescriptor{label:Some("synthetic vertex observation"),source:wgpu::ShaderSource::Wgsl(
            "@fragment fn observe(@location(2) value:vec4<f32>)->@location(0) vec4<f32>{return value;}".into())});
        let attributes: Vec<_> = (0..16)
            .map(|i| wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: u64::from(i) * 16,
                shader_location: i,
            })
            .collect();
        let make = |module: &wgpu::ShaderModule, entry: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("NVI1 vector pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vs,
                    entry_point: Some("vertexMain"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 256,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba32Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let witness = make(&observe, "observe");
        let fragment = make(&fs, "fragmentMain");
        let mut textures = Vec::new();
        for color in [
            [64, 128, 192, 255],
            [128, 64, 32, 128],
            [32, 64, 128, 255],
            [255, 255, 255, 255],
        ] {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &color,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            textures.push(texture.create_view(&Default::default()));
        }
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        Self {
            device,
            queue,
            witness,
            fragment,
            layout,
            textures,
            sampler,
        }
    }
    fn draw(&self, block: &Block, values: [[f32; 4]; 16], witness: bool) -> [f32; 4] {
        let data = block.encode(0).checked();
        assert_eq!(host_nvi1::validate(&data, [false; 4]), Ok(()));
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &data,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let mut bindings = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }];
        for i in 0..4 {
            bindings.push(wgpu::BindGroupEntry {
                binding: 1 + i * 2,
                resource: wgpu::BindingResource::TextureView(&self.textures[i as usize]),
            });
            bindings.push(wgpu::BindGroupEntry {
                binding: 2 + i * 2,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            });
        }
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &bindings,
        });
        let mut vertices = Vec::new();
        for position in [
            [-1., -1., 0.5, 1.],
            [1., -1., 0.5, 1.],
            [-1., 1., 0.5, 1.],
            [-1., 1., 0.5, 1.],
            [1., -1., 0.5, 1.],
            [1., 1., 0.5, 1.],
        ] {
            let mut v = values;
            v[0] = position;
            for attribute in v {
                for x in attribute {
                    vertices.extend(x.to_le_bytes());
                }
            }
        }
        let vertex = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &vertices,
                usage: wgpu::BufferUsages::VERTEX,
            });
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64 * 1024,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: -2.,
                            g: -3.,
                            b: -4.,
                            a: -5.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(if witness {
                &self.witness
            } else {
                &self.fragment
            });
            pass.set_bind_group(0, &group, &[]);
            pass.set_vertex_buffer(0, vertex.slice(..));
            pass.draw(0..6, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(1024),
                    rows_per_image: Some(64),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (send, done) = mpsc::channel();
        output
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).checked());
        self.device.poll(wgpu::Maintain::Wait);
        done.recv().checked().checked();
        let bytes = output.slice(..).get_mapped_range();
        let at = 32 * 1024 + 32 * 16;
        std::array::from_fn(|i| {
            f32::from_le_bytes(bytes[at + i * 4..at + i * 4 + 4].try_into().checked())
        })
    }
}

// Raw instruction writer used only by vectors, independent of the production
// decoder's implementation. Sources are (hardware mux, register, swizzle).
fn ins(
    mac: u32,
    ilu: u32,
    s: [(u32, u32, u32); 3],
    temp: u32,
    mask: u32,
    out: u32,
    unit: u32,
    input: u32,
) -> [u32; 4] {
    let a = s[0];
    let b = s[1];
    let c = s[2];
    [
        0,
        mac << 21 | ilu << 25 | input << 9 | a.2,
        a.1 << 28 | a.0 << 26 | b.2 << 17 | b.1 << 13 | b.0 << 11 | c.2 << 2 | (c.1 >> 2),
        (c.1 & 3) << 30
            | c.0 << 28
            | mask << 24
            | temp << 20
            | if ilu != 0 { mask << 16 } else { 0 }
            | if out != 255 {
                15 << 12 | 1 << 11 | out << 3 | unit << 2
            } else {
                0
            },
    ]
}
fn mov_input(input: u32, temp: u32, out: u32) -> [u32; 4] {
    ins(
        1,
        0,
        [(2, 0, 0x1b), (0, 0, 0), (0, 0, 0)],
        temp,
        if temp == 15 { 0 } else { 15 },
        out,
        0,
        input,
    )
}
fn block(mut tail: Vec<[u32; 4]>) -> Block {
    let mut p = vec![
        mov_input(0, 15, 0),
        mov_input(3, 0, 255),
        mov_input(4, 1, 255),
        mov_input(5, 2, 255),
    ];
    p.append(&mut tail);
    p.last_mut().checked()[3] |= 1;
    Block::programmed(&Program::decode(&p, VertexMode::CLIP).checked())
}
fn close(label: &str, actual: [f32; 4], expected: [f32; 4], tolerance: f32) {
    for i in 0..4 {
        if expected[i].is_infinite() {
            assert_eq!(actual[i], expected[i], "{label} lane {i}");
        } else {
            assert!(
                (actual[i] - expected[i]).abs() <= tolerance,
                "{label} lane {i}: {actual:?} != {expected:?}"
            );
        }
    }
}
fn reference_combiners(fixtures: &[CombinerRegisters], values: [[f32; 4]; 16]) -> Vec<[f32; 4]> {
    let mut data = Vec::new();
    for r in fixtures {
        data.push(2);
        for x in [
            r.control,
            r.final_inputs[0],
            r.final_inputs[1],
            r.final_c0,
            r.final_c1,
        ] {
            data.extend(x.to_le_bytes());
        }
        for i in 0..8 {
            for x in [
                r.rgb_inputs[i],
                r.alpha_inputs[i],
                r.rgb_outputs[i],
                r.alpha_outputs[i],
                r.c0[i],
                r.c1[i],
            ] {
                data.extend(x.to_le_bytes());
            }
        }
        let mut registers = [[0.; 4]; 16];
        registers[4] = values[3];
        registers[5] = values[4];
        for i in 8..12 {
            registers[i] = [0., 0., 0., 1.];
        }
        registers[12][3] = 1.;
        for word in registers {
            for x in word {
                data.extend(f32::to_le_bytes(x));
            }
        }
    }
    let executable = std::env::var_os("TAKEDAWN_NVI1_ORACLE").expect("run check_shaders.py");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .checked();
    let mut input = child.stdin.take().checked();
    let writer = std::thread::spawn(move || {
        input.write_all(&data).checked();
    });
    let result = child.wait_with_output().checked();
    writer.join().checked();
    assert!(result.status.success());
    assert_eq!(result.stdout.len(), fixtures.len() * 16);
    result
        .stdout
        .chunks_exact(16)
        .map(|b| {
            std::array::from_fn(|i| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().checked()))
        })
        .collect()
}
fn main() {
    let mut args: Vec<_> = std::env::args().collect();
    let software_diagnostic = args.get(1).map(String::as_str) == Some("--software-diagnostic");
    if software_diagnostic {
        args.remove(1);
    }
    assert_eq!(args.len(), 3);
    let gpu = Gpu::new(
        &std::fs::read_to_string(&args[1]).checked(),
        &std::fs::read_to_string(&args[2]).checked(),
        software_diagnostic,
    );
    let mut v = [[0.; 4]; 16];
    v[3] = [1., 2., 3., 4.];
    v[4] = [5., 6., 7., 8.];
    v[5] = [9., 10., 11., 12.];
    let source = [(1, 0, 0x1b), (1, 1, 0x1b), (1, 2, 0x1b)];
    let expected = [
        [0., 0., 0., 1.],
        [1., 2., 3., 4.],
        [5., 12., 21., 32.],
        [10., 12., 14., 16.],
        [14., 22., 32., 44.],
        [38.; 4],
        [46.; 4],
        [70.; 4],
        [1., 12., 3., 8.],
        [1., 2., 3., 4.],
        [5., 6., 7., 8.],
        [1.; 4],
        [0.; 4],
    ];
    for mac in 0..13 {
        let b = block(vec![ins(
            mac,
            0,
            source,
            15,
            0,
            if mac == 0 { 255 } else { 9 },
            0,
            0,
        )]);
        close(
            &format!("MAC {mac}"),
            gpu.draw(&b, v, true),
            expected[mac as usize],
            0.0,
        );
    }
    v[5] = [2., 3., 4., 5.];
    let expected = [
        [0., 0., 0., 1.],
        [2., 3., 4., 5.],
        [0.5; 4],
        [0.5; 4],
        [std::f32::consts::FRAC_1_SQRT_2; 4],
        [4., 0., 4., 1.],
        [1., 1., 1., 1.],
        [1., 2., 243., 1.],
    ];
    for ilu in 0..8 {
        let b = block(vec![ins(
            0,
            ilu,
            source,
            15,
            0,
            if ilu == 0 { 255 } else { 9 },
            1,
            0,
        )]);
        // Only RSQ and LIT receive explicit numeric tolerances in these vectors.
        close(
            &format!("ILU {ilu}"),
            gpu.draw(&b, v, true),
            expected[ilu as usize],
            if ilu == 4 {
                0.000001
            } else if ilu == 7 {
                0.0001
            } else {
                0.0
            },
        );
    }
    let mut paired = ins(
        3,
        1,
        [(1, 0, 0x1b), (0, 0, 0), (1, 0, 0x1b)],
        0,
        15,
        255,
        0,
        0,
    );
    paired[3] |= 15 << 16;
    let b = block(vec![
        paired,
        ins(1, 0, [(1, 1, 0x1b), (0, 0, 0), (0, 0, 0)], 15, 0, 9, 0, 0),
    ]);
    close(
        "paired old values",
        gpu.draw(&b, v, true),
        [1., 2., 3., 4.],
        0.0,
    );
    for swizzle in 0..256 {
        let b = block(vec![ins(
            1,
            0,
            [(1, 0, swizzle), (0, 0, 0), (0, 0, 0)],
            15,
            0,
            9,
            0,
            0,
        )]);
        let expected = std::array::from_fn(|i| v[3][((swizzle >> (6 - 2 * i)) & 3) as usize]);
        close("all swizzles", gpu.draw(&b, v, true), expected, 0.0);
    }
    // ARL saturation and relative constant indexing, including infinities.
    for (x, address) in [
        (0., 0),
        (-0., 0),
        (-0.0005, 0),
        (-0.0015, -1),
        (1., 1),
        (191.9, 191),
        (192., 192),
        (10000., 192),
        (-192., -192),
        (-10000., -192),
        (f32::INFINITY, 192),
        (f32::NEG_INFINITY, -192),
    ] {
        let arl = ins(
            13,
            0,
            [(1, 0, 0x1b), (0, 0, 0), (0, 0, 0)],
            15,
            0,
            255,
            0,
            0,
        );
        let mut relative = ins(1, 0, [(3, 0, 0x1b), (0, 0, 0), (0, 0, 0)], 15, 0, 9, 0, 0);
        relative[1] |= 96 << 13;
        relative[3] |= 2;
        let mut b = block(vec![arl, relative]);
        for i in 0..192 {
            b.set_vertex_constant(i, [i as f32; 4]).checked();
        }
        v[3] = [x; 4];
        close(
            "ARL bounded address",
            gpu.draw(&b, v, true),
            [(96i32 + address).clamp(0, 191) as f32; 4],
            0.0,
        );
    }
    v[3] = [1.; 4];
    let mut paired = ins(
        13,
        1,
        [(1, 0, 0x1b), (0, 0, 0), (3, 0, 0x1b)],
        15,
        0,
        9,
        1,
        0,
    );
    paired[1] |= 96 << 13;
    paired[3] |= 2;
    let mut b = block(vec![paired]);
    b.set_vertex_constant(96, [96.; 4]).checked();
    b.set_vertex_constant(97, [97.; 4]).checked();
    close(
        "ARL paired old address",
        gpu.draw(&b, v, true),
        [96.; 4],
        0.0,
    );
    // A dynamically produced NaN reaches ARL although every supplied value is
    // finite. This clips execution; it is not whole-draw preflight rejection.
    v[3] = [0.; 4];
    v[5] = [0.; 4];
    let make_infinite = ins(0, 2, [(0, 0, 0), (0, 0, 0), (1, 2, 0x1b)], 1, 15, 255, 0, 0);
    let multiply = ins(
        2,
        0,
        [(1, 0, 0x1b), (1, 1, 0x1b), (0, 0, 0)],
        0,
        15,
        255,
        0,
        0,
    );
    let arl = ins(
        13,
        0,
        [(1, 0, 0x1b), (0, 0, 0), (0, 0, 0)],
        15,
        0,
        255,
        0,
        0,
    );
    close(
        "dynamic ARL NaN clips",
        gpu.draw(&block(vec![make_infinite, multiply, arl]), v, true),
        [-2., -3., -4., -5.],
        0.0,
    );
    // Final combiner: RGB=V0, alpha=V0.a, no general stages/textures.
    let registers = CombinerRegisters {
        final_inputs: [4, 0x1400],
        ..CombinerRegisters::default()
    };
    let p = [mov_input(0, 15, 0), {
        let mut x = mov_input(3, 15, 3);
        x[3] |= 1;
        x
    }];
    let mut b = Block::programmed(&Program::decode(&p, VertexMode::CLIP).checked())
        .with_combiners(&registers)
        .checked();
    v[3] = [0.25, 0.5, 0.75, 1.];
    close("final combiner", gpu.draw(&b, v, false), v[3], 0.0);
    let mut fixtures = Vec::new();
    for mapping in 0..8 {
        for output in [0, 1, 2, 3, 4, 6] {
            let mut r = CombinerRegisters {
                control: 1,
                final_inputs: [12, 0x1c00],
                ..CombinerRegisters::default()
            };
            // A=map(V0), B=1, C=D=0; independent RGB and blue-selected alpha.
            r.rgb_inputs[0] = (4 | mapping << 5) << 24 | 0x20 << 16;
            r.alpha_inputs[0] = (4 | mapping << 5) << 24 | 0x20 << 16;
            r.rgb_outputs[0] = 12 << 8 | output << 15;
            r.alpha_outputs[0] = 12 << 8 | output << 15;
            fixtures.push(r);
        }
    }
    let expected = reference_combiners(&fixtures, v);
    for (r, expected) in fixtures.iter().zip(expected) {
        let b = Block::programmed(&Program::decode(&p, VertexMode::CLIP).checked())
            .with_combiners(r)
            .checked();
        close(
            "unchanged C combiner differential",
            gpu.draw(&b, v, false),
            expected,
            0.0,
        );
    }
    for mask in 0..16 {
        b.set_texture(
            0,
            &Texture {
                mode: 5,
                clip: mask,
                ..Texture::default()
            },
        )
        .checked();
        let expected = if mask == 0 {
            v[3]
        } else {
            [-2., -3., -4., -5.]
        };
        close(
            "clip masks with positive coordinates",
            gpu.draw(&b, v, false),
            expected,
            0.0,
        );
    }
    println!("PASS: 13 MAC, 8 ILU, paired issue, 256 swizzles, 12 ARL addresses, paired ARL, dynamic NaN clipping, 48 unchanged-C combiner differentials, final combiner and 16 clip masks");
    if software_diagnostic {
        println!("SOFTWARE DIAGNOSTIC ONLY: hardware qualification remains unexecuted.");
    }
    println!("Synthetic execution only: no gameplay, physical input, sound, pacing or performance qualification.");
}

// Production errors intentionally implement no formatting/system interfaces.
trait Checked<T> {
    fn checked(self) -> T;
}
impl<T, E> Checked<T> for Result<T, E> {
    fn checked(self) -> T {
        match self {
            Ok(value) => value,
            Err(_) => panic!("unexpected component error"),
        }
    }
}
impl<T> Checked<T> for Option<T> {
    fn checked(self) -> T {
        self.expect("expected component result")
    }
}
