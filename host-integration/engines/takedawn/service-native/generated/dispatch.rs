fn route_0(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
    address: u32,
) -> Result<(), i32> {
    match address {
        80848u32 => crate::game::sub_00013BD0(native_state, guest_ram),
        80912u32 => crate::game::sub_00013C10(native_state, guest_ram),
        80992u32 => crate::game::sub_00013C60(native_state, guest_ram),
        81056u32 => crate::game::sub_00013CA0(native_state, guest_ram),
        81680u32 => crate::game::sub_00013F10(native_state, guest_ram),
        87296u32 => crate::game::sub_00015500(native_state, guest_ram),
        87312u32 => crate::game::sub_00015510(native_state, guest_ram),
        87344u32 => crate::game::sub_00015530(native_state, guest_ram),
        87440u32 => crate::game::sub_00015590(native_state, guest_ram),
        87456u32 => crate::game::sub_000155A0(native_state, guest_ram),
        87472u32 => crate::game::sub_000155B0(native_state, guest_ram),
        87488u32 => crate::game::sub_000155C0(native_state, guest_ram),
        _ => Err(-5),
    }
}
fn route_1(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
    address: u32,
) -> Result<(), i32> {
    match address {
        87504u32 => crate::game::sub_000155D0(native_state, guest_ram),
        87520u32 => crate::game::sub_000155E0(native_state, guest_ram),
        87536u32 => crate::game::sub_000155F0(native_state, guest_ram),
        87552u32 => crate::game::sub_00015600(native_state, guest_ram),
        87568u32 => crate::game::sub_00015610(native_state, guest_ram),
        87600u32 => crate::game::sub_00015630(native_state, guest_ram),
        87616u32 => crate::game::sub_00015640(native_state, guest_ram),
        87648u32 => crate::game::sub_00015660(native_state, guest_ram),
        87664u32 => crate::game::sub_00015670(native_state, guest_ram),
        87680u32 => crate::game::sub_00015680(native_state, guest_ram),
        87696u32 => crate::game::sub_00015690(native_state, guest_ram),
        87712u32 => crate::game::sub_000156A0(native_state, guest_ram),
        _ => Err(-5),
    }
}
fn route_2(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
    address: u32,
) -> Result<(), i32> {
    match address {
        87728u32 => crate::game::sub_000156B0(native_state, guest_ram),
        87744u32 => crate::game::sub_000156C0(native_state, guest_ram),
        87760u32 => crate::game::sub_000156D0(native_state, guest_ram),
        87776u32 => crate::game::sub_000156E0(native_state, guest_ram),
        _ => Err(-5),
    }
}
pub fn invoke(
    native_state: &mut crate::registers::Registers,
    guest_ram: &mut crate::memory::Memory,
    address: u32,
) -> Result<(), i32> {
    if !native_state.valid() {
        return Err(-1);
    }
    match route_0(native_state, guest_ram, address) {
        Err(-5) => {}
        result => return result,
    }
    match route_1(native_state, guest_ram, address) {
        Err(-5) => {}
        result => return result,
    }
    match route_2(native_state, guest_ram, address) {
        Err(-5) => {}
        result => return result,
    }
    Err(-5)
}
