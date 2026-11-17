// Ordinary admitted setup, routing and protection witnesses.
use crate::calls_runtime;
use crate::cases;
use crate::gamepkg;
use crate::heap_service;
use crate::imports;
use crate::kernel_data;
use crate::kernel_pool;
use crate::layout;
use crate::memory;
use crate::registers;
pub fn imports_memory() -> Result<(memory::Memory, [u64; 3]), i32> {
    imports_memory_mode(false)
}
pub fn imports_memory_services() -> Result<(memory::Memory, [u64; 3]), i32> {
    imports_memory_mode(true)
}
fn imports_memory_mode(services: bool) -> Result<(memory::Memory, [u64; 3]), i32> {
    let root = memory::Memory::new()?;
    let topology = if services {
        layout::Layout::layout_services(&root)?
    } else {
        layout::Layout::layout_current(&root)?
    };
    let banks = topology.layout_banks();
    let mut req = std::vec::Vec::new();
    req.extend(u64::to_le_bytes(banks[0]));
    req.extend(u64::to_le_bytes(0));
    req.extend(u64::to_le_bytes(67108864));
    gamepkg::local_call(4, &req, &mut [])?;
    req.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2, &req, &mut [])?;
    let mut ram = topology.layout_view(&root)?;
    ram.fill(0, 0xa5, 67108864)?;
    Ok((ram, banks))
}
pub fn imports_owner(
    ram: &mut memory::Memory,
    banks: [u64; 3],
    base: u32,
    physical: bool,
) -> Result<u64, i32> {
    imports_owner_mode(ram, banks, base, 67108864, physical, false)
}
pub fn imports_owner_services(
    ram: &mut memory::Memory,
    banks: [u64; 3],
    base: u32,
    physical: bool,
) -> Result<u64, i32> {
    imports_owner_mode(ram, banks, base, 67108864, physical, true)
}
pub fn imports_owner_virtual(
    ram: &mut memory::Memory,
    banks: [u64; 3],
    base: u32,
) -> Result<u64, i32> {
    let heap_limit = ram.memory_guest_arena(base)?;
    imports_owner_mode(ram, banks, base, heap_limit, true, true)
}
fn imports_owner_mode(
    ram: &mut memory::Memory,
    banks: [u64; 3],
    base: u32,
    heap_limit: u32,
    physical: bool,
    services: bool,
) -> Result<u64, i32> {
    let inbox = heap_service::heap_inbox()?;
    let mut req = std::vec::Vec::new();
    req.extend(u32::to_le_bytes(0));
    req.extend(u64::to_le_bytes(inbox));
    req.extend(u32::to_le_bytes(0));
    let mut result = [0u8; 8];
    gamepkg::local_call(10, &req, &mut result)?;
    let job = u64::from_le_bytes(result);
    let mut client = heap_service::HeapClient::heap_client(inbox, job)?;
    client.heap_initialize(ram.call_handle(), banks[0], base, heap_limit)?;
    if physical {
        if services {
            if client.heap_contiguous_initialize(banks[1]) != Err(-1)
                || client.heap_contiguous_services(banks[0]) != Err(-4)
                || client.heap_contiguous_services(banks[1] | 0x100000000) != Err(-4)
            {
                return Err(-3);
            }
            client.heap_contiguous_services(banks[1])?;
            if client.heap_contiguous_services(banks[1]) != Err(-1) {
                return Err(-3);
            }
        } else {
            client.heap_contiguous_initialize(banks[1])?;
        }
    }
    let mut owner = Some(client);
    ram.heap_install(&mut owner)?;
    Ok(job)
}
pub fn imports_environment(
    physical: bool,
) -> Result<(memory::Memory, kernel_data::Data, u64, u64, u64), i32> {
    let (mut ram, banks) = imports_memory()?;
    let job = imports_owner(&mut ram, banks, 0x1000000, physical)?;
    let data = kernel_data::Data::data_initialize(&mut ram, 0x700000, 123, 0, &[], None)?;
    Ok((ram, data, job, banks[0], banks[1]))
}
pub fn imports_cleanup(ram: &mut memory::Memory, job: u64) -> Result<(), i32> {
    ram.kernel_clear()?;
    ram.heap_close()?;
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut result = [0u8; 8];
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut result) {
            Ok(8) => break,
            Err(-2) => {
                if gamepkg::local_mailbox_receive(pause, 1, &mut byte) != Err(-2) {
                    return Err(-3);
                }
            }
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
    gamepkg::local_mailbox_release(pause)?;
    if u64::from_le_bytes(result) != 0 {
        return Err(-3);
    }
    match gamepkg::local_record_release(ram.call_handle()) {
        Ok(()) | Err(-4) => Ok(()),
        Err(code) => Err(code),
    }
}
pub fn imports_case(mode: u32) -> Result<std::vec::Vec<u8>, i32> {
    let case = cases::Case::import_case(mode)?;
    let (mut ram, data, job, _, _) = imports_environment(true)?;
    case.import_seed(&mut ram)?;
    let pages = case.import_states(&ram)?;
    let prepared = imports::Imports::imports_scan(&ram, case.table, &data, &case.remap)?;
    prepared.imports_publish(&mut ram)?;
    if prepared.imports_publish(&mut ram) != Err(-1) {
        return Err(-3);
    }
    let shape = prepared.imports_layout();
    let (_, record) = gamepkg::local_record_snapshot(ram.kernel_handle(), 1540)?;
    let mut witness = std::vec::Vec::new();
    for value in [mode, shape[0], shape[1], shape[2]] {
        witness.extend(u32::to_le_bytes(value));
    }
    let length = 64 + shape[1] as usize * 4 + if mode == 4 { 0 } else { 4 };
    let mut bytes = std::vec::Vec::new();
    bytes.resize(length, 0u8);
    ram.read_bytes(case.table - 64, &mut bytes)?;
    witness.extend(bytes);
    let mut callable = 378u32;
    let mut data_slot = 378u32;
    for slot in 0..378u32 {
        let ordinal = if slot < shape[2] {
            let at = 24 + slot as usize * 4;
            u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
        } else {
            0
        };
        let function = kernel_pool::kernel_supported(ordinal).is_ok() && slot < shape[2];
        if function {
            if kernel_pool::kernel_ordinal(&ram, slot)? != ordinal {
                return Err(-3);
            }
        } else if kernel_pool::kernel_ordinal(&ram, slot) != Err(-5) {
            return Err(-3);
        }
        let args = if function {
            kernel_pool::kernel_supported(ordinal)?
        } else {
            0
        };
        witness.extend(u32::to_le_bytes(ordinal));
        witness.extend(u32::to_le_bytes(args));
        witness.extend([u8::from(function)]);
        if ordinal == 14 && callable == 378 {
            callable = slot;
        }
        if ordinal != 0 && !function && data_slot == 378 {
            data_slot = slot;
        }
    }
    let mut state = registers::Registers::new();
    state.eax = 0x87654321;
    state.ebx = 0x19370001;
    state.esp = 0x50000;
    state.xmm7 = [0xffff0000, 0, 1, 0xffffffff];
    let before = state.encode();
    if data_slot != 378 {
        if calls_runtime::invoke_jump(&mut state, &mut ram, 0xfe000000 + data_slot * 4) != Err(-5)
            || state.encode() != before
        {
            return Err(-3);
        }
    }
    let mut result = [u32::MAX, 0u32, 0];
    if callable != 378 {
        ram.write_u32(0x50000, 0x12345678)?;
        ram.write_u32(0x50004, 64)?;
        let target = 0xfe000000 + callable * 4 + (mode & 3);
        if mode & 1 == 0 {
            calls_runtime::invoke_call(&mut state, &mut ram, target, 0x50000)?;
        } else {
            calls_runtime::invoke_jump(&mut state, &mut ram, target)?;
        }
        let mut expected = registers::Registers::new();
        expected.decode(&before)?;
        expected.eax = 0x1000000;
        expected.esp = 0x50008;
        if state.encode() != expected.encode() {
            return Err(-3);
        }
        result = [callable, state.eax, state.esp];
    }
    let (_, _, traces) = ram.call_snapshot()?;
    if traces != u64::from(callable != 378 && mode & 1 == 0) {
        return Err(-3);
    }
    for value in result {
        witness.extend(u32::to_le_bytes(value));
    }
    for page in &pages {
        if gamepkg::local_memory_page_states(page[1], page[2], 1)? != [page[3] as u8] {
            return Err(-3);
        }
    }
    let mut client = ram.heap_take()?;
    let counts = client.heap_status();
    ram.heap_put(client);
    for value in counts? {
        witness.extend(u32::to_le_bytes(value));
    }
    imports_cleanup(&mut ram, job)?;
    Ok(witness)
}
