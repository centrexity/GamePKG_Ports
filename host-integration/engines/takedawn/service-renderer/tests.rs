//! Synthetic maintainer tests. These are not game source or gameplay evidence.
use std::io::Write;
use std::process::{Command, Stdio};
use takedawn_nvi1::{Block, CombinerRegisters, Program, Texture, VertexMode, ABSENT, BYTES};

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().checked()))
        .collect()
}
fn host(block: &Block, cube: u32) {
    let bytes = block.encode(cube).checked();
    assert_eq!(
        host_nvi1::validate(
            &bytes,
            [cube & 1 != 0, cube & 2 != 0, cube & 4 != 0, cube & 8 != 0]
        ),
        Ok(())
    );
}
fn raw(mac: u32, ilu: u32) -> [u32; 4] {
    // Source A/B/C all v0, identity; final, MAC writes oPos, ILU R1.
    [
        0,
        mac << 21 | ilu << 25 | 0x1b,
        2 << 26 | 0x1b << 17 | 2 << 11 | 0x1b << 2,
        2 << 28
            | 1 << 11
            | 1
            | if mac != 0 && mac != 13 { 15 << 12 } else { 0 }
            | if ilu != 0 { 15 << 16 } else { 0 },
    ]
}

#[test]
fn all_supported_opcode_pairs_produce_host_admissible_data() {
    for mac in 0..14 {
        for ilu in 0..8 {
            let p = Program::decode(&[raw(mac, ilu)], VertexMode::SCREEN).checked();
            assert_eq!(p.instruction_count(), 1);
            assert_eq!(p.input_mask(), u32::from(mac != 0 || ilu != 0));
            let instruction = p.instruction(0).checked();
            if mac == 0 || mac == 13 {
                assert_eq!(instruction[2], ABSENT);
            }
            if ilu == 0 {
                assert_eq!(instruction[3], ABSENT);
            }
            host(&Block::programmed(&p), 0);
        }
    }
    host(&Block::fixed(), 0);
}

#[test]
fn canonical_wire_offsets_masks_endianness_and_texture_dimensions() {
    let mut b = Block::fixed();
    b.set_vertex_constant(191, [1.0, -2.0, 3.0, 4.0]).checked();
    b.set_alpha(127.0 / 255.0, 3, true).checked();
    b.set_fog([0.25, 0.5, 0.75, 1.0], true, true).checked();
    let t = Texture {
        mode: 3,
        signed_channels: 15,
        ..Texture::default()
    };
    b.set_texture(2, &t).checked();
    assert!(b.encode(0).is_err());
    host(&b, 4);
    let bytes = b.encode(4).checked();
    assert_eq!(&bytes[..4], b"NVI1");
    assert_eq!(bytes.len(), BYTES);
    let w = words(&bytes);
    assert_eq!(w[6], 0x1e19);
    assert_eq!(w[7], 6);
    assert_eq!(
        &w[780..784],
        &[
            1f32.to_bits(),
            (-2f32).to_bits(),
            3f32.to_bits(),
            4f32.to_bits()
        ]
    );
    assert_eq!(w[3194], 3);
    assert!(w[804..2980].iter().all(|v| *v == 0));
}

#[test]
fn raw_reserved_and_unsupported_encodings_never_become_nop() {
    for mac in [14, 15] {
        let e = Program::decode(&[raw(mac, 0)], VertexMode::CLIP)
            .err()
            .checked();
        assert!(!e.unsupported);
    }
    let mut w = raw(1, 0);
    w[3] &= !(1 << 11);
    assert!(
        Program::decode(&[w], VertexMode::CLIP)
            .err()
            .checked()
            .unsupported
    );
    let mut w = raw(1, 0);
    w[1] = (w[1] & !(255 << 13)) | (192 << 13);
    w[2] = (w[2] & !(3 << 26)) | (3 << 26);
    assert!(Program::decode(&[w], VertexMode::CLIP).is_err());
    let mut w = raw(1, 0);
    w[2] = (w[2] & !(3 << 26)) | (1 << 26) | (13 << 28);
    assert!(Program::decode(&[w], VertexMode::CLIP).is_err());
    let mut w = raw(1, 0);
    w[3] &= !1;
    assert!(Program::decode(&[w; 137], VertexMode::CLIP).is_err());
    let mut p = vec![w; 136];
    p[135][3] |= 1;
    assert_eq!(
        Program::decode(&p, VertexMode::CLIP)
            .checked()
            .instruction_count(),
        136
    );
    assert!(Program::decode(&[], VertexMode::CLIP).is_err());
}

#[test]
fn combiner_flags_registers_and_atomic_error_behavior() {
    let mut r = CombinerRegisters {
        control: 8 | (1 << 8) | (1 << 12) | (1 << 16),
        ..CombinerRegisters::default()
    };
    r.rgb_outputs[7] = (12 << 8) | (4 << 15) | (1 << 13) | (1 << 19);
    r.alpha_outputs[7] = 12 << 8 | 4 << 15;
    r.final_inputs[1] = 0xe0;
    r.c0[7] = 0x80402010;
    let b = Block::fixed();
    let accepted = b.with_combiners(&r).checked();
    host(&accepted, 0);
    let w = words(&accepted.encode(0).checked());
    assert_eq!(w[3], 0xe0);
    assert_eq!(w[5], 8);
    assert_eq!(w[3116 + 7 * 4], (64f32 / 255.0).to_bits());
    let before = b.encode(0).checked();
    for map in [5, 7] {
        r.rgb_outputs[0] = map << 15;
        assert!(b.with_combiners(&r).is_err());
        assert_eq!(b.encode(0).checked(), before);
    }
    r.rgb_outputs[0] = 0;
    r.rgb_inputs[0] = 6 << 24;
    assert!(b.with_combiners(&r).is_err());
    r.rgb_inputs[0] = 0;
    r.final_inputs[1] = 14 << 24;
    assert!(b.with_combiners(&r).is_err());
    r.final_inputs[1] = 1;
    assert!(b.with_combiners(&r).is_err());
    r.final_inputs[1] = 0;
    r.control = 9;
    assert!(b.with_combiners(&r).is_err());
}

#[test]
fn texture_clip_masks_predecessors_parameters_and_failed_updates() {
    let mut b = Block::fixed()
        .with_combiners(&CombinerRegisters::default())
        .checked();
    for slot in 0..4 {
        for mask in 0..16 {
            b.set_texture(
                slot,
                &Texture {
                    mode: 5,
                    clip: mask,
                    ..Texture::default()
                },
            )
            .checked();
            host(&b, 0);
        }
    }
    let before = b.encode(0).checked();
    for t in [
        Texture {
            mode: 8,
            ..Texture::default()
        },
        Texture {
            mode: 5,
            clip: 16,
            ..Texture::default()
        },
        Texture {
            mode: 1,
            clip: 1,
            ..Texture::default()
        },
        Texture {
            mode: 6,
            previous: 1,
            ..Texture::default()
        },
        Texture {
            lod_bias: f32::INFINITY,
            ..Texture::default()
        },
    ] {
        assert!(b.set_texture(1, &t).is_err());
        assert_eq!(b.encode(0).checked(), before);
    }
    assert!(
        b.set_texture(
            0,
            &Texture {
                mode: 2,
                ..Texture::default()
            }
        )
        .err()
        .checked()
        .unsupported
    );
    assert!(
        b.set_texture(
            0,
            &Texture {
                mode: 6,
                ..Texture::default()
            }
        )
        .err()
        .checked()
        .unsupported
    );
    b.set_texture(
        1,
        &Texture {
            mode: 7,
            ..Texture::default()
        },
    )
    .checked();
    host(&b, 0);
    assert!(b.set_vertex_constant(0, [f32::NAN; 4]).is_err());
    assert!(b.set_vertex_constant(192, [0.0; 4]).is_err());
    let mut basic = Block::fixed();
    basic
        .set_texture(
            0,
            &Texture {
                mode: 5,
                ..Texture::default()
            },
        )
        .checked();
    assert!(basic.encode(0).err().checked().unsupported);
}

fn random(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}
fn exchange(request: Vec<u8>) -> Vec<u32> {
    let executable = std::env::var_os("TAKEDAWN_NVI1_ORACLE").expect("run check_nvi1.sh");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .checked();
    let mut input = child.stdin.take().checked();
    let writer = std::thread::spawn(move || {
        input.write_all(&request).checked();
    });
    let output = child.wait_with_output().checked();
    writer.join().checked();
    assert!(output.status.success());
    words(&output.stdout)
}
fn send(request: &mut Vec<u8>, v: u32) {
    request.extend(v.to_le_bytes());
}

#[test]
fn differential_vertex_data_matches_unchanged_pinned_c_parser() {
    let mut seed = 0x81578131;
    let mut request = Vec::new();
    let mut expected = Vec::new();
    for _ in 0..4096 {
        let mac = random(&mut seed) % 14;
        let ilu = random(&mut seed) % 8;
        let r1 = random(&mut seed) & 0x001fffff;
        let mut r2 = random(&mut seed);
        let mut r3 = random(&mut seed);
        r2 = (r2 & !(15 << 28 | 15 << 13 | 3))
            | ((random(&mut seed) % 13) << 28)
            | ((random(&mut seed) % 13) << 13);
        let c = random(&mut seed) % 13;
        r2 |= c >> 2;
        r3 = (r3 & !(3 << 30 | 15 << 20)) | ((c & 3) << 30) | ((random(&mut seed) % 13) << 20);
        let mut w = [
            random(&mut seed),
            r1 | mac << 21 | ilu << 25,
            r2,
            r3 | 1 | 1 << 11,
        ];
        w[1] = (w[1] & !(255 << 13)) | ((random(&mut seed) % 192) << 13);
        w[3] = (w[3] & !(255 << 3)) | ((random(&mut seed) % 11 + 2) << 3);
        // Select only actual output IDs; address 2 is not an output.
        let output = [0, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12][(random(&mut seed) % 11) as usize];
        w[3] = (w[3] & !(255 << 3)) | output << 3;
        if (w[3] & 4 == 0 && (mac == 0 || mac == 13)) || (w[3] & 4 != 0 && ilu == 0) {
            w[3] &= !(15 << 12);
        }
        let p = Program::decode(&[w], VertexMode::SCREEN).checked();
        request.push(0);
        for x in w {
            send(&mut request, x);
        }
        expected.extend(p.instruction(0).checked());
        expected.push(p.input_mask());
        host(&Block::programmed(&p), 0);
    }
    assert_eq!(exchange(request), expected);
}

#[test]
fn differential_combiner_data_matches_unchanged_pinned_c_parser() {
    let mut seed = 0xc2711831;
    let mut request = Vec::new();
    let mut expected = Vec::new();
    let registers = [0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13];
    for _ in 0..1024 {
        let mut r = CombinerRegisters {
            control: random(&mut seed) % 9 | random(&mut seed) & 0x11100,
            ..CombinerRegisters::default()
        };
        for k in 0..7 {
            let value = registers[(random(&mut seed) % 12) as usize] | (random(&mut seed) & 0x30);
            r.final_inputs[usize::from(k >= 4)] |= value << (24 - 8 * (k % 4));
        }
        r.final_inputs[1] |= random(&mut seed) & 0xe0;
        for i in 0..8 {
            for k in 0..4 {
                r.rgb_inputs[i] |= (registers[(random(&mut seed) % 12) as usize]
                    | (random(&mut seed) & 0xf0))
                    << (24 - 8 * k);
                r.alpha_inputs[i] |= (registers[(random(&mut seed) % 12) as usize]
                    | (random(&mut seed) & 0xf0))
                    << (24 - 8 * k);
            }
            for alpha in [false, true] {
                let value = registers[(random(&mut seed) % 12) as usize]
                    | registers[(random(&mut seed) % 12) as usize] << 4
                    | registers[(random(&mut seed) % 12) as usize] << 8
                    | [0, 1, 2, 3, 4, 6][(random(&mut seed) % 6) as usize] << 15
                    | (random(&mut seed) & if alpha { 1 << 14 } else { 0xc7000 });
                if alpha {
                    r.alpha_outputs[i] = value;
                } else {
                    r.rgb_outputs[i] = value;
                }
            }
        }
        let b = Block::fixed().with_combiners(&r).checked();
        host(&b, 0);
        let w = words(&b.encode(0).checked());
        request.push(1);
        send(&mut request, r.control);
        for x in r.final_inputs {
            send(&mut request, x);
        }
        for i in 0..8 {
            for x in [
                r.rgb_inputs[i],
                r.alpha_inputs[i],
                r.rgb_outputs[i],
                r.alpha_outputs[i],
            ] {
                send(&mut request, x);
            }
        }
        expected.extend([w[5], w[8], w[9], w[10], w[3]]);
        expected.extend(&w[2980..3108]);
        expected.extend(&w[3108..3115]);
    }
    assert_eq!(exchange(request), expected);
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
