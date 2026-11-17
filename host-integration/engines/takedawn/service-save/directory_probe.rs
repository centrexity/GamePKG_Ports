use crate::gamepkg;
use crate::kernel_directories;
use crate::registers;
use crate::setup;

fn directory_expect(code: i32, condition: bool) -> Result<(), i32> {
    setup::mixed_need(code, condition)
}

fn directory_guest_name(file_fixture: &mut setup::Fixture, name: &[u8]) -> Result<(), i32> {
    file_fixture.ram.write_u32(0x10300, 0)?;
    file_fixture.ram.write_u32(0x10304, 0x10320)?;
    file_fixture.ram.write_u32(0x10308, 0x40)?;
    file_fixture.ram.write_u16(0x10320, name.len() as u16)?;
    file_fixture.ram.write_u16(0x10322, name.len() as u16 + 1)?;
    file_fixture.ram.write_u32(0x10324, 0x10340)?;
    file_fixture.ram.write_bytes(0x10340, name)
}

fn directory_guest_call(
    disk_directories: &mut kernel_directories::SaveDirectories,
    file_fixture: &mut setup::Fixture,
    ordinal: u32,
    values: &[u32],
    expected: Result<u32, i32>,
) -> Result<(), i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = 0x10400;
    cpu.eax = 0x12345678;
    cpu.ecx = 0xabcdef01;
    file_fixture.ram.write_u32(cpu.esp, 0x87654321)?;
    for (index, value) in values.iter().enumerate() {
        file_fixture
            .ram
            .write_u32(cpu.esp + 4 + index as u32 * 4, *value)?;
    }
    let original = cpu.encode();
    let result = disk_directories.save_directory_invoke(
        &file_fixture.table,
        &mut file_fixture.ram,
        &mut cpu,
        ordinal,
    );
    match expected {
        Ok(status) => {
            let mut wanted = registers::Registers::new();
            wanted.decode(&original)?;
            wanted.eax = status;
            wanted.esp += 4 + values.len() as u32 * 4;
            directory_expect(
                950,
                result == Ok(())
                    && cpu.encode() == wanted.encode()
                    && file_fixture.ram.kernel_caller() == 0x87654321,
            )
        }
        Err(error) => directory_expect(951, result == Err(error) && cpu.encode() == original),
    }
}

pub fn directory_probe_run(save_grant: u64, disc_grant: u64) -> Result<(), i32> {
    let mut file_fixture = setup::Fixture::mixed_fixture_new()?;
    directory_expect(
        952,
        kernel_directories::SaveDirectories::save_directories_new(
            &file_fixture.ram,
            &file_fixture.table,
            disc_grant,
        )
        .is_err(),
    )?;
    let mut disk_directories = kernel_directories::SaveDirectories::save_directories_new(
        &file_fixture.ram,
        &file_fixture.table,
        save_grant,
    )?;
    let quota_file = gamepkg::file_create_new(save_grant, "quota.seed")?;
    gamepkg::file_resize(quota_file, 16385)?;

    directory_guest_name(&mut file_fixture, b"\\Device\\Harddisk0\\partition1\\")?;
    let open_dir_args = [0x10100, 0x100001, 0x10300, 0x10108, 3, 0x800021];
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        202,
        &open_dir_args,
        Ok(0),
    )?;
    let root_directory = file_fixture.ram.read_u32(0x10100)?;
    directory_expect(
        953,
        root_directory != 0 && file_fixture.ram.read_u64(0x10108)? == 0x100000000,
    )?;

    let mut volume_args = [root_directory, 0x10108, 0x10120, 23, 3];
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        218,
        &volume_args,
        Err(-5),
    )?;
    volume_args[3] = 24;
    file_fixture.ram.write_u32(0x10138, 0x11aa22bb)?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        218,
        &volume_args,
        Ok(0),
    )?;
    directory_expect(
        954,
        file_fixture.ram.read_u64(0x10120)? == 4
            && file_fixture.ram.read_u64(0x10128)? == 2
            && file_fixture.ram.read_u32(0x10130)? == 32
            && file_fixture.ram.read_u32(0x10134)? == 512
            && file_fixture.ram.read_u32(0x10138)? == 0x11aa22bb
            && file_fixture.ram.read_u64(0x10108)? == 24u64 << 32,
    )?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        187,
        &[root_directory],
        Ok(0),
    )?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        218,
        &volume_args,
        Err(-4),
    )?;

    let create_dir_args = [0x10100, 0x100001, 0x10300, 0x10108, 0, 128, 3, 3, 0x4021];
    directory_guest_name(&mut file_fixture, b"\\Device\\Harddisk0\\partition1\\UDATA")?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        202,
        &open_dir_args,
        Ok(0xc0000034),
    )?;
    directory_expect(
        955,
        file_fixture.ram.read_u32(0x10100)? == root_directory
            && file_fixture.ram.read_u64(0x10108)? == 0xc0000034,
    )?;
    file_fixture.ram.write_u32(0x10300, root_directory)?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        190,
        &create_dir_args,
        Err(-5),
    )?;
    file_fixture.ram.write_u32(0x10300, 0)?;
    let mut bad_directory_args = create_dir_args;
    bad_directory_args[3] = 0x12000;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        190,
        &bad_directory_args,
        Err(-4),
    )?;
    directory_expect(
        956,
        gamepkg::save_directory(save_grant, "UDATA", false) == Ok(0),
    )?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        190,
        &create_dir_args,
        Ok(0),
    )?;
    directory_expect(957, file_fixture.ram.read_u64(0x10108)? == 0x200000000)?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        190,
        &create_dir_args,
        Ok(0),
    )?;
    directory_expect(958, file_fixture.ram.read_u64(0x10108)? == 0x100000000)?;
    for forbidden in [
        b"\\Device\\Harddisk0\\partition2\\UDATA".as_slice(),
        b"\\Device\\Harddisk0\\partition1\\UDATA\\..",
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005c",
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005b:stream",
    ] {
        directory_guest_name(&mut file_fixture, forbidden)?;
        directory_guest_call(
            &mut disk_directories,
            &mut file_fixture,
            190,
            &create_dir_args,
            Err(-5),
        )?;
    }
    directory_guest_name(
        &mut file_fixture,
        b"\\DEVICE\\HARDDISK0\\PARTITION1\\udata\\4541005B",
    )?;
    directory_guest_call(
        &mut disk_directories,
        &mut file_fixture,
        190,
        &create_dir_args,
        Ok(0),
    )?;
    directory_expect(
        959,
        gamepkg::save_directory(save_grant, "UDATA/4541005b", false) == Ok(1),
    )?;

    disk_directories.save_directories_drain(&file_fixture.ram, &file_fixture.table)?;
    gamepkg::file_close(quota_file)?;
    gamepkg::file_remove(save_grant, "quota.seed")?;
    file_fixture.mixed_fixture_close()
}
