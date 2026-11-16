pub fn sub_001D20AC(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    (native_state.ecx) = guest_ram.read_u32(((native_state.esp).wrapping_add(4 as u32)) as u32)?;
    let mut _tsc: u64 = crate::kernel_pool::kernel_tsc_read(guest_ram)?;
    (native_state.eax) = _tsc as u32;
    (native_state.edx) = (_tsc >> 32 as i32) as u32;
    guest_ram.write_u32((native_state.ecx) as u32, (native_state.eax))?;
    guest_ram.write_u32(
        ((native_state.ecx).wrapping_add(4 as u32)) as u32,
        (native_state.edx),
    )?;
    (native_state.eax) = 0 as u32;
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.eax) = (native_state.eax).wrapping_add(1);
    _fa = (native_state.eax) & 0xffffffff as u32;
    _fas = _fa as i32;
    _fb = (_fa == 0x80000000 as u32) as i32 as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(8 as u32);
    Ok(())
}
pub fn sub_001FC960(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _pv: u32 = (native_state.ebx);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    let mut _tsc: u64 = crate::kernel_pool::kernel_tsc_read(guest_ram)?;
    (native_state.eax) = _tsc as u32;
    (native_state.edx) = (_tsc >> 32 as i32) as u32;
    (native_state.ebx) = 0x41b960 as u32;
    guest_ram.write_u32((native_state.ebx) as u32, (native_state.eax))?;
    guest_ram.write_u32(
        ((native_state.ebx).wrapping_add(4 as u32)) as u32,
        (native_state.edx),
    )?;
    (native_state.eax) = 1 as u32;
    (native_state.ebx) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_00356AC0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _tsc: u64 = crate::kernel_pool::kernel_tsc_read(guest_ram)?;
    (native_state.eax) = _tsc as u32;
    (native_state.edx) = (_tsc >> 32 as i32) as u32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_001FC930(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    _fb = 8 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_sub(8 as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    let mut _pv: u32 = (native_state.ebx);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    let mut _tsc: u64 = crate::kernel_pool::kernel_tsc_read(guest_ram)?;
    (native_state.eax) = _tsc as u32;
    (native_state.edx) = (_tsc >> 32 as i32) as u32;
    (native_state.ebx) = (native_state.esp).wrapping_add(4 as u32);
    guest_ram.write_u32((native_state.ebx) as u32, (native_state.eax))?;
    guest_ram.write_u32(
        ((native_state.ebx).wrapping_add(4 as u32)) as u32,
        (native_state.edx),
    )?;
    (native_state.eax) = guest_ram.read_u32(((native_state.esp).wrapping_add(4 as u32)) as u32)?;
    (native_state.ebx) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    _fb = 8 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_add(8 as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
pub fn sub_001FC980(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
) -> Result<(), i32> {
    let mut _fa: u32 = 0 as u32;
    let mut _fb: u32 = 0 as u32;
    let mut _fas: i32 = 0 as i32;
    let mut _fbs: i32 = 0 as i32;
    _fb = 8 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_sub(8 as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    let mut _pv: u32 = (native_state.ebx);
    (native_state.esp) = (native_state.esp).wrapping_sub(4 as u32);
    guest_ram.write_u32((native_state.esp) as u32, _pv)?;
    let mut _tsc: u64 = crate::kernel_pool::kernel_tsc_read(guest_ram)?;
    (native_state.eax) = _tsc as u32;
    (native_state.edx) = (_tsc >> 32 as i32) as u32;
    (native_state.ebx) = (native_state.esp).wrapping_add(4 as u32);
    guest_ram.write_u32((native_state.ebx) as u32, (native_state.eax))?;
    guest_ram.write_u32(
        ((native_state.ebx).wrapping_add(4 as u32)) as u32,
        (native_state.edx),
    )?;
    (native_state.ebx) = guest_ram.read_u32((native_state.esp) as u32)?;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    _fb = 8 as i32 as u32 & 0xffffffff as u32;
    _fbs = _fb as i32;
    (native_state.esp) = (native_state.esp).wrapping_add(8 as u32);
    _fa = (native_state.esp) & 0xffffffff as u32;
    _fas = _fa as i32;
    (native_state.esp) = (native_state.esp).wrapping_add(4 as u32);
    Ok(())
}
