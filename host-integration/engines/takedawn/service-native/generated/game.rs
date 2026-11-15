// Maintainer-generated from pinned Takedawn; see LICENSE and provenance.json.
pub fn sub_00013BD0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut ebp: u32 = 0 as u32;
    ebp = (native_state.ebp);
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    let mut _pv: u32 = ebp;
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    ebp = (native_state.esp);
    (native_state.ebp) = ebp;
    (native_state.seh_ebp) = ebp;
    (native_state.esp) = (native_state.esp) & 0xfffffff0 as u32;
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fb = 0xc as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_sub(0xc as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.xmm1) = crate::vector::load(guest_ram, (native_state.ecx))?;
    (native_state.edx) = guest_ram.read_u32((ebp.wrapping_add(8 as u32)) as u32)?;
    (native_state.xmm0) = crate::vector::load(guest_ram, (native_state.edx))?;
    let mut _pv_0: u32 = (native_state.esi);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv_0)?;
    (native_state.xmm1) = crate::vector::add((native_state.xmm1), (native_state.xmm0));
    crate::vector::store(guest_ram, (native_state.ecx), (native_state.xmm1))?;
    (native_state.esi) = guest_ram.read_u32((native_state.ecx) as u32)?;
    (native_state.edx) = (native_state.eax);
    guest_ram.write_u32((native_state.edx) as u32, (native_state.esi))?;
    (native_state.esi) = guest_ram.read_u32(((native_state.ecx).wrapping_add(4 as u32)) as u32)?;
    guest_ram.write_u32(
        ((native_state.edx).wrapping_add(4 as u32)) as u32,
        (native_state.esi),
    )?;
    (native_state.esi) = guest_ram.read_u32(((native_state.ecx).wrapping_add(8 as u32)) as u32)?;
    guest_ram.write_u32(
        ((native_state.edx).wrapping_add(8 as u32)) as u32,
        (native_state.esi),
    )?;
    (native_state.ecx) =
        guest_ram.read_u32(((native_state.ecx).wrapping_add(0xc as u32)) as u32)?;
    guest_ram.write_u32(
        ((native_state.edx).wrapping_add(0xc as u32)) as u32,
        (native_state.ecx),
    )?;
    (native_state.esi) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = ebp;
    ebp = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = (native_state.esp).wrapping_add(8 as u32);
    Ok(())
}
pub fn sub_00013C10(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut ebp: u32 = 0 as u32;
    ebp = (native_state.ebp);
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    let mut _pv: u32 = ebp;
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    ebp = (native_state.esp);
    (native_state.ebp) = ebp;
    (native_state.seh_ebp) = ebp;
    (native_state.esp) = (native_state.esp) & 0xfffffff0 as u32;
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fb = 0x20 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_sub(0x20 as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.xmm0) = crate::vector::load(guest_ram, (native_state.eax))?;
    (native_state.xmm0) = crate::vector::multiply((native_state.xmm0), (native_state.xmm0));
    (native_state.xmm1) = (native_state.xmm0);
    (native_state.xmm1) =
        crate::vector::shuffle((native_state.xmm1), (native_state.xmm0), 0x39 as u32);
    {
        let converted_lane_value = crate::vector::add_float(
            crate::vector::float((native_state.xmm0), 0usize)?,
            crate::vector::float((native_state.xmm1), 0usize)?,
        );
        crate::vector::set_float(&mut (native_state.xmm0), 0usize, converted_lane_value)?;
    };
    (native_state.xmm2) = (native_state.xmm1);
    (native_state.xmm2) =
        crate::vector::shuffle((native_state.xmm2), (native_state.xmm1), 0x39 as u32);
    {
        let converted_lane_value = crate::vector::add_float(
            crate::vector::float((native_state.xmm0), 0usize)?,
            crate::vector::float((native_state.xmm2), 0usize)?,
        );
        crate::vector::set_float(&mut (native_state.xmm0), 0usize, converted_lane_value)?;
    };
    crate::vector::store(
        guest_ram,
        (native_state.esp).wrapping_add(0x10 as u32),
        (native_state.xmm0),
    )?;
    {
        let converted_lane_value = (crate::vector::float((native_state.xmm0), 0usize)?).sqrt();
        crate::vector::set_float(&mut (native_state.xmm0), 0usize, converted_lane_value)?;
    };
    guest_ram.write_f32(
        ((native_state.esp).wrapping_add(0x10 as u32)) as u32,
        crate::vector::float((native_state.xmm0), 0usize)?,
    )?;
    (native_state.xmm0) =
        crate::vector::load(guest_ram, (native_state.esp).wrapping_add(0x10 as u32))?;
    (native_state.ecx) = (native_state.esp).wrapping_add(0xc as u32);
    guest_ram.write_f32(
        (native_state.ecx) as u32,
        crate::vector::float((native_state.xmm0), 0usize)?,
    )?;
    (native_state.xmm0) = crate::vector::scalar(
        guest_ram.read_f32(((native_state.esp).wrapping_add(0xc as u32)) as u32)?,
    );
    (native_state.esp) = ebp;
    ebp = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00013C60(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut ebp: u32 = 0 as u32;
    ebp = (native_state.ebp);
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    let mut _pv: u32 = ebp;
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    ebp = (native_state.esp);
    (native_state.ebp) = ebp;
    (native_state.seh_ebp) = ebp;
    (native_state.esp) = (native_state.esp) & 0xfffffff0 as u32;
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fb = 0x10 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_sub(0x10 as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.xmm1) = crate::vector::load(guest_ram, (native_state.eax))?;
    (native_state.xmm0) = crate::vector::load(guest_ram, (native_state.ecx))?;
    (native_state.xmm0) = crate::vector::multiply((native_state.xmm0), (native_state.xmm1));
    (native_state.xmm1) = (native_state.xmm0);
    (native_state.xmm1) =
        crate::vector::shuffle((native_state.xmm1), (native_state.xmm0), 0x39 as u32);
    (native_state.xmm2) = (native_state.xmm1);
    {
        let converted_lane_value = crate::vector::add_float(
            crate::vector::float((native_state.xmm0), 0usize)?,
            crate::vector::float((native_state.xmm1), 0usize)?,
        );
        crate::vector::set_float(&mut (native_state.xmm0), 0usize, converted_lane_value)?;
    };
    (native_state.xmm2) =
        crate::vector::shuffle((native_state.xmm2), (native_state.xmm1), 0x39 as u32);
    {
        let converted_lane_value = crate::vector::add_float(
            crate::vector::float((native_state.xmm0), 0usize)?,
            crate::vector::float((native_state.xmm2), 0usize)?,
        );
        crate::vector::set_float(&mut (native_state.xmm0), 0usize, converted_lane_value)?;
    };
    (native_state.edx) = (native_state.esp).wrapping_add(0xc as u32);
    guest_ram.write_f32(
        (native_state.edx) as u32,
        crate::vector::float((native_state.xmm0), 0usize)?,
    )?;
    (native_state.xmm0) = crate::vector::scalar(
        guest_ram.read_f32(((native_state.esp).wrapping_add(0xc as u32)) as u32)?,
    );
    (native_state.esp) = ebp;
    ebp = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00013CA0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut ebp: u32 = 0 as u32;
    ebp = (native_state.ebp);
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    let mut _pv: u32 = ebp;
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    ebp = (native_state.esp);
    (native_state.ebp) = ebp;
    (native_state.seh_ebp) = ebp;
    (native_state.esp) = (native_state.esp) & 0xfffffff0 as u32;
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fb = 0x18 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_sub(0x18 as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    let mut _pv_0: u32 = (native_state.esi);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv_0)?;
    let mut _pv_1: u32 = (native_state.edi);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv_1)?;
    (native_state.esi) = guest_ram.read_u32((ebp.wrapping_add(8 as u32)) as u32)?;
    (native_state.edi) = guest_ram.read_u32((ebp.wrapping_add(0xc as u32)) as u32)?;
    (native_state.xmm0) = crate::vector::scalar(guest_ram.read_f32((native_state.edi) as u32)?);
    (native_state.xmm0) =
        crate::vector::shuffle((native_state.xmm0), (native_state.xmm0), 0 as u32);
    (native_state.xmm0) = crate::vector::multiply(
        (native_state.xmm0),
        crate::vector::load(guest_ram, (native_state.esi))?,
    );
    (native_state.xmm0) = crate::vector::add(
        (native_state.xmm0),
        crate::vector::load(guest_ram, (native_state.esi).wrapping_add(0x30 as u32))?,
    );
    (native_state.xmm1) = crate::vector::scalar(
        guest_ram.read_f32(((native_state.edi).wrapping_add(4 as u32)) as u32)?,
    );
    (native_state.xmm1) =
        crate::vector::shuffle((native_state.xmm1), (native_state.xmm1), 0 as u32);
    (native_state.xmm1) = crate::vector::multiply(
        (native_state.xmm1),
        crate::vector::load(guest_ram, (native_state.esi).wrapping_add(0x10 as u32))?,
    );
    (native_state.xmm0) = crate::vector::add((native_state.xmm0), (native_state.xmm1));
    (native_state.xmm1) = crate::vector::scalar(
        guest_ram.read_f32(((native_state.edi).wrapping_add(8 as u32)) as u32)?,
    );
    (native_state.xmm1) =
        crate::vector::shuffle((native_state.xmm1), (native_state.xmm1), 0 as u32);
    (native_state.xmm1) = crate::vector::multiply(
        (native_state.xmm1),
        crate::vector::load(guest_ram, (native_state.esi).wrapping_add(0x20 as u32))?,
    );
    (native_state.xmm0) = crate::vector::add((native_state.xmm0), (native_state.xmm1));
    crate::vector::store(
        guest_ram,
        (native_state.esp).wrapping_add(0x10 as u32),
        (native_state.xmm0),
    )?;
    (native_state.edx) =
        guest_ram.read_u32(((native_state.esp).wrapping_add(0x10 as u32)) as u32)?;
    (native_state.ecx) = (native_state.eax);
    guest_ram.write_u32((native_state.ecx) as u32, (native_state.edx))?;
    (native_state.edx) =
        guest_ram.read_u32(((native_state.esp).wrapping_add(0x14 as u32)) as u32)?;
    guest_ram.write_u32(
        ((native_state.ecx).wrapping_add(4 as u32)) as u32,
        (native_state.edx),
    )?;
    (native_state.edx) =
        guest_ram.read_u32(((native_state.esp).wrapping_add(0x18 as u32)) as u32)?;
    guest_ram.write_u32(
        ((native_state.ecx).wrapping_add(8 as u32)) as u32,
        (native_state.edx),
    )?;
    (native_state.edx) =
        guest_ram.read_u32(((native_state.esp).wrapping_add(0x1c as u32)) as u32)?;
    (native_state.edi) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    guest_ram.write_u32(
        ((native_state.ecx).wrapping_add(0xc as u32)) as u32,
        (native_state.edx),
    )?;
    (native_state.esi) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = ebp;
    ebp = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00013F10(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _flags: i32 = 0 as i32;
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    let mut _pv: u32 = (native_state.ebx);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    (native_state.ebx) = (native_state.ebx) & 0xffffff00 as u32
        | guest_ram.read_u8((0x4a1c74 as i32 as u32) as u32)? as u32;
    (native_state.edx) = (native_state.edx) & 0xffffff00 as u32 | 0 as i32 as u8 as u32;
    _fa = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    _fa = ((native_state.ebx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    _fbs = _fb as i8 as i32;
    (native_state.ecx) = (native_state.ecx) & 0xffffff00 as u32 | 1 as i32 as u8 as u32;
    's_71: {
        if _fa == _fb {
            break 's_71;
        }
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(2 as u32)) as u32,
            ((native_state.ecx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(3 as u32)) as u32,
            ((native_state.ecx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            (native_state.eax) as u32,
            ((native_state.edx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(1 as u32)) as u32,
            ((native_state.edx) & 0xff as u32) as u8,
        )?;
    }
    _fa = guest_ram.read_u8((0x4a1c75 as i32 as u32) as u32)? as u32 & 0xff as u32;
    _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    _fbs = _fb as i8 as i32;
    's_96: {
        if _fa == _fb {
            break 's_96;
        }
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(2 as u32)) as u32,
            ((native_state.ecx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            (native_state.eax) as u32,
            ((native_state.edx) & 0xff as u32) as u8,
        )?;
    }
    _fa = guest_ram.read_u8((0x4a1c78 as i32 as u32) as u32)? as u32 & 0xff as u32;
    _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    _fbs = _fb as i8 as i32;
    's_125: {
        if _fa == _fb {
            break 's_125;
        }
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(2 as u32)) as u32,
            ((native_state.edx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(3 as u32)) as u32,
            ((native_state.edx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            (native_state.eax) as u32,
            ((native_state.ecx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(1 as u32)) as u32,
            ((native_state.ecx) & 0xff as u32) as u8,
        )?;
    }
    _fa = guest_ram.read_u8((0x4a1c79 as i32 as u32) as u32)? as u32 & 0xff as u32;
    _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    _fbs = _fb as i8 as i32;
    's_150: {
        if _fa == _fb {
            break 's_150;
        }
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(3 as u32)) as u32,
            ((native_state.edx) & 0xff as u32) as u8,
        )?;
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(1 as u32)) as u32,
            ((native_state.ecx) & 0xff as u32) as u8,
        )?;
    }
    _fa =
        guest_ram.read_u8(((native_state.eax).wrapping_add(2 as u32)) as u32)? as u32 & 0xff as u32;
    _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    _fbs = _fb as i8 as i32;
    's_212: {
        's_206: {
            if _fa == _fb {
                break 's_206;
            }
            _fa = guest_ram.read_u8((0x4a1c76 as i32 as u32) as u32)? as u32 & 0xff as u32;
            _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
            _fas = _fa as i8 as i32;
            _fbs = _fb as i8 as i32;
            's_184: {
                if _fa != _fb {
                    break 's_184;
                }
                guest_ram.write_u8(
                    (native_state.eax) as u32,
                    ((native_state.edx) & 0xff as u32) as u8,
                )?;
                (native_state.ecx) = (native_state.ecx) & 0xffffff00 as u32 | 0 as i32 as u8 as u32;
                _fa = ((native_state.ecx) & 0xff as u32) as u8 as u32 & 0xff as u32;
                _fas = _fa as i8 as i32;
                break 's_212;
            }
            guest_ram.write_u8(
                (native_state.eax) as u32,
                ((native_state.ecx) & 0xff as u32) as u8,
            )?;
            break 's_212;
        }
        (native_state.ecx) = (native_state.ecx) & 0xffffff00 as u32
            | guest_ram.read_u8((0x4a1c77 as i32 as u32) as u32)? as u32;
    }
    _fa =
        guest_ram.read_u8(((native_state.eax).wrapping_add(3 as u32)) as u32)? as u32 & 0xff as u32;
    _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    _fbs = _fb as i8 as i32;
    (native_state.ebx) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    's_257: {
        if _fa == _fb {
            break 's_257;
        }
        _fa = ((native_state.ecx) & 0xff as u32) as u8 as u32 & 0xff as u32;
        _fb = ((native_state.edx) & 0xff as u32) as u8 as u32 & 0xff as u32;
        _fas = _fa as i8 as i32;
        _fbs = _fb as i8 as i32;
        (native_state.ecx) = (native_state.ecx) & 0xffffff00 as u32
            | (if _fa != _fb { 1 as i32 } else { 0 as i32 }) as u8 as u32;
        guest_ram.write_u8(
            ((native_state.eax).wrapping_add(1 as u32)) as u32,
            ((native_state.ecx) & 0xff as u32) as u8,
        )?;
    }
    guest_ram.write_u8(
        (0x4a1c74 as i32 as u32) as u32,
        ((native_state.edx) & 0xff as u32) as u8,
    )?;
    guest_ram.write_u8(
        (0x4a1c75 as i32 as u32) as u32,
        ((native_state.edx) & 0xff as u32) as u8,
    )?;
    guest_ram.write_u8(
        (0x4a1c76 as i32 as u32) as u32,
        ((native_state.edx) & 0xff as u32) as u8,
    )?;
    guest_ram.write_u8(
        (0x4a1c77 as i32 as u32) as u32,
        ((native_state.edx) & 0xff as u32) as u8,
    )?;
    guest_ram.write_u8(
        (0x4a1c78 as i32 as u32) as u32,
        ((native_state.edx) & 0xff as u32) as u8,
    )?;
    guest_ram.write_u8(
        (0x4a1c79 as i32 as u32) as u32,
        ((native_state.edx) & 0xff as u32) as u8,
    )?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015500(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    guest_ram.write_u32(
        ((native_state.ecx).wrapping_add(0x238 as u32)) as u32,
        0x19 as u32,
    )?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015510(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.ecx) = guest_ram.read_u32((native_state.eax) as u32)?;
    (native_state.edx) = guest_ram.read_u32(((native_state.eax).wrapping_add(4 as u32)) as u32)?;
    let mut _pv: u32 = (native_state.esi);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    (native_state.esi) = (native_state.ecx);
    (native_state.esi) = ((native_state.esi) as i32 >> (0x10 as u32 & 31 as u32)) as u32;
    _fa = (native_state.esi) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.ecx) = (native_state.ecx) << 0x10 as i32;
    _fa = (native_state.ecx) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fb = (native_state.edx) & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esi) = (native_state.esi).wrapping_add((native_state.edx));
    _fa = (native_state.esi) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fb = (native_state.esi) & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.ecx) = (native_state.ecx).wrapping_add((native_state.esi));
    _fa = (native_state.ecx) & 0xffffffff as u32;
    _fas = _fa as i32;
    guest_ram.write_u32((native_state.eax) as u32, (native_state.ecx))?;
    _fb = (native_state.edx) & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.ecx) = (native_state.ecx).wrapping_add((native_state.edx));
    _fa = (native_state.ecx) & 0xffffffff as u32;
    _fas = _fa as i32;
    guest_ram.write_u32(
        ((native_state.eax).wrapping_add(4 as u32)) as u32,
        (native_state.ecx),
    )?;
    (native_state.eax) = guest_ram.read_u32((native_state.eax) as u32)?;
    (native_state.esi) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015530(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _flags: i32 = 0 as i32;
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    _fb = 4 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.eax) = (native_state.eax).wrapping_add(4 as u32);
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fas = _fa as i32;
    let mut _pv: u32 = (native_state.esi);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    (native_state.esi) = (native_state.esi) & 0xffff0000 as u32
        | guest_ram.read_u16((native_state.ecx) as u32)? as u32;
    (native_state.edx) = (native_state.eax).wrapping_add(0x10 as u32);
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fb = (native_state.edx) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fbs = _fb as i32;
    guest_ram.write_u16(
        (native_state.eax) as u32,
        ((native_state.esi) & 0xffff as u32) as u16,
    )?;
    's_133: {
        's_123: {
            if _fa >= _fb {
                break 's_123;
            }
            loop {
                _fa = guest_ram.read_u16((native_state.ecx) as u32)? as u32 & 0xffff as u32;
                _fb = 0 as i32 as u32 & 0xffff as u32;
                _fas = _fa as i16 as i32;
                _fbs = _fb as i16 as i32;
                if _fa == _fb {
                    break 's_133;
                }
                (native_state.esi) = (native_state.esi) & 0xffff0000 as u32
                    | guest_ram.read_u16(((native_state.ecx).wrapping_add(2 as u32)) as u32)?
                        as u32;
                _fb = 2 as i32 as u32 & 0xffffffff as u32;
                _fbs = _fb as i32;
                (native_state.ecx) = (native_state.ecx).wrapping_add(2 as u32);
                _fa = (native_state.ecx) & 0xffffffff as u32;
                _fas = _fa as i32;
                _fb = 2 as i32 as u32 & 0xffffffff as u32;
                _fbs = _fb as i32;
                (native_state.eax) = (native_state.eax).wrapping_add(2 as u32);
                _fa = (native_state.eax) & 0xffffffff as u32;
                _fas = _fa as i32;
                _fa = (native_state.eax) & 0xffffffff as u32;
                _fb = (native_state.edx) & 0xffffffff as u32;
                _fas = _fa as i32;
                _fbs = _fb as i32;
                guest_ram.write_u16(
                    (native_state.eax) as u32,
                    ((native_state.esi) & 0xffff as u32) as u16,
                )?;
                if _fa >= _fb {
                    break 's_123;
                }
            }
        }
        guest_ram.write_u16((native_state.edx) as u32, 0 as u16)?;
    }
    (native_state.esi) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015590(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.eax) = (native_state.eax) | 0xffffffff as u32;
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000155A0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _fp_value: f64 = guest_ram.read_f32((0x3b168c as i32 as u32) as u32)? as f64;
    (native_state.fp_top) =
        (((native_state.fp_top) as u32).wrapping_add(7 as u32) & 7 as u32) as i32;
    (native_state.fp_stack)[(native_state.fp_top) as usize] = _fp_value;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000155B0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.eax) = (native_state.eax) & 0xffffff00 as u32 | 0 as i32 as u8 as u32;
    _fa = ((native_state.eax) & 0xff as u32) as u8 as u32 & 0xff as u32;
    _fas = _fa as i8 as i32;
    (native_state.esp) = (native_state.esp).wrapping_add(8 as u32);
    Ok(())
}
pub fn sub_000155C0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = (native_state.eax) & 0xffffff00 as u32 | 1 as i32 as u8 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(12 as u32);
    Ok(())
}
pub fn sub_000155D0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _flags: i32 = 0 as i32;
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.edx) =
        guest_ram.read_u32(((native_state.ecx).wrapping_add(0x40 as u32)) as u32)?;
    (native_state.eax) = 0 as u32;
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fa = (native_state.edx) & 0xffffffff as u32;
    _fb = 2 as i32 as u32 & 0xffffffff as u32;
    _fas = _fa as i32;
    _fbs = _fb as i32;
    (native_state.eax) = (native_state.eax) & 0xffffff00 as u32
        | (if _fa == _fb { 1 as i32 } else { 0 as i32 }) as u8 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000155E0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 7 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000155F0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _flags: i32 = 0 as i32;
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.edx) =
        guest_ram.read_u32(((native_state.ecx).wrapping_add(0x40 as u32)) as u32)?;
    (native_state.eax) = 0 as u32;
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fa = (native_state.edx) & 0xffffffff as u32;
    _fb = 3 as i32 as u32 & 0xffffffff as u32;
    _fas = _fa as i32;
    _fbs = _fb as i32;
    (native_state.eax) = (native_state.eax) & 0xffffff00 as u32
        | (if _fa == _fb { 1 as i32 } else { 0 as i32 }) as u8 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015600(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 6 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015610(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _flags: i32 = 0 as i32;
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.eax) =
        guest_ram.read_u32(((native_state.ecx).wrapping_add(0x40 as u32)) as u32)?;
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fb = 1 as i32 as u32 & 0xffffffff as u32;
    _fas = _fa as i32;
    _fbs = _fb as i32;
    's_68: {
        if _fa == _fb {
            break 's_68;
        }
        _fa = (native_state.eax) & 0xffffffff as u32;
        _fb = 4 as i32 as u32 & 0xffffffff as u32;
        _fas = _fa as i32;
        _fbs = _fb as i32;
        if _fa == _fb {
            break 's_68;
        }
        (native_state.eax) = 0 as u32;
        _fa = (native_state.eax) & 0xffffffff as u32;
        _fas = _fa as i32;
        (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
        return Ok(());
    }
    (native_state.eax) = 1 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015630(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 0xd as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015640(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 0xf as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015650(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _flags: i32 = 0 as i32;
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.ecx) = guest_ram.read_u32((0x463b14 as i32 as u32) as u32)?;
    (native_state.eax) = 0 as u32;
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fa = (native_state.ecx) & 0xffffffff as u32;
    _fb = 1 as i32 as u32 & 0xffffffff as u32;
    _fas = _fa as i32;
    _fbs = _fb as i32;
    (native_state.eax) = (native_state.eax) & 0xffffff00 as u32
        | (if if if 4usize < 4usize { 4usize } else { 4usize } == 1usize {
            _fas as u32 as u8 as i8 as i32
        } else if if 4usize < 4usize { 4usize } else { 4usize } == 2usize {
            _fas as u32 as u16 as i16 as i32
        } else {
            _fas as u32 as i32
        } > if if 4usize < 4usize { 4usize } else { 4usize } == 1usize {
            _fbs as u32 as u8 as i8 as i32
        } else if if 4usize < 4usize { 4usize } else { 4usize } == 2usize {
            _fbs as u32 as u16 as i16 as i32
        } else {
            _fbs as u32 as i32
        } {
            1 as i32
        } else {
            0 as i32
        }) as u8 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015660(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 8 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015670(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    guest_ram.write_u8(
        ((native_state.ecx).wrapping_add(0x49 as u32)) as u32,
        1 as u8,
    )?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015680(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    guest_ram.write_u8(
        ((native_state.ecx).wrapping_add(0x4a as u32)) as u32,
        1 as u8,
    )?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00015690(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 0x10 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000156A0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 0x11 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000156B0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 0x12 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000156C0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) =
        guest_ram.read_u32(((native_state.ecx).wrapping_add(0x68 as u32)) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000156D0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) = 0x13 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_000156E0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    (native_state.eax) =
        guest_ram.read_u32(((native_state.ecx).wrapping_add(0x64 as u32)) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
