// Checked marker entrypoint for a startup which opted into file services.
use crate::gamepkg;
use crate::kernel_directories;
use crate::kernel_file;
use crate::kernel_regular;
use crate::kernel_startup;
use crate::memory;
use crate::registers;
use crate::xbe;

pub struct StartupFiles<'a, 'b> {
    owner: &'a kernel_startup::Startup,
    image: &'a xbe::Image<'b>,
    rom_adapter: kernel_file::RomKernel,
    private_directories: Option<kernel_directories::SaveDirectories>,
    regular_files: Option<kernel_regular::RegularFiles<'a>>,
}

impl<'a, 'b> StartupFiles<'a, 'b> {
    pub fn startup_files_new(
        owner: &'a kernel_startup::Startup,
        image: &'a xbe::Image<'b>,
        ram: &memory::Memory,
    ) -> Result<Self, i32> {
        Self::startup_files_bind(owner, image, ram, None)
    }

    pub fn startup_files_with_regular(
        owner: &'a kernel_startup::Startup,
        image: &'a xbe::Image<'b>,
        ram: &memory::Memory,
        save_session: &'a mut kernel_regular::SaveSession,
    ) -> Result<Self, i32> {
        Self::startup_files_bind(owner, image, ram, Some(save_session))
    }

    fn startup_files_bind(
        owner: &'a kernel_startup::Startup,
        image: &'a xbe::Image<'b>,
        ram: &memory::Memory,
        save_session: Option<&'a mut kernel_regular::SaveSession>,
    ) -> Result<Self, i32> {
        let table = owner.startup_rom_table(ram, image)?;
        let rom_adapter = kernel_file::RomKernel::rom_kernel_new(ram, table)?;
        let storage_grant = owner.startup_private_save(ram, image)?;
        let private_directories = if storage_grant == 0 {
            None
        } else {
            Some(kernel_directories::SaveDirectories::save_directories_new(
                ram,
                table,
                storage_grant,
            )?)
        };
        let expected_worker = owner.startup_regular_worker(ram, image)?;
        let regular_files = match save_session {
            Some(session) => {
                if session.regular_session_ids()? != [storage_grant, expected_worker] {
                    return Err(-4);
                }
                Some(kernel_regular::RegularFiles::regular_files_new(session)?)
            }
            None if expected_worker == 0 => None,
            _ => return Err(-4),
        };
        Ok(Self {
            owner,
            image,
            rom_adapter,
            private_directories,
            regular_files,
        })
    }

    pub fn startup_files_call(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut registers::Registers,
        marker: u32,
    ) -> Result<(), i32> {
        let ordinal = self.owner.startup_rom_ordinal(ram, self.image, marker)?;
        let table = self.owner.startup_rom_table(ram, self.image)?;
        if !cpu.valid() || cpu.esp == 0 {
            return Err(-1);
        }
        let token_at = cpu.esp.checked_add(4).ok_or(-1)?;
        let route = match ordinal {
            190 | 202 if self.private_directories.is_some() => {
                startup_open_route(ram, cpu, ordinal, self.regular_files.is_some())?
            }
            211 | 236 => 2,
            218 => 1,
            187 | 219 => {
                let token = ram.read_u32(token_at)?;
                if let Some(regular) = self.regular_files.as_ref() {
                    if regular.regular_files_owns(token) {
                        return self
                            .regular_files
                            .as_mut()
                            .ok_or(-5)?
                            .regular_files_invoke(ram, table, cpu, ordinal);
                    }
                }
                match self.private_directories.as_ref() {
                    Some(directories)
                        if ordinal == 187 && directories.save_directory_owns(token) =>
                    {
                        1
                    }
                    _ => 0,
                }
            }
            _ => 0,
        };
        if route == 1 {
            return self
                .private_directories
                .as_mut()
                .ok_or(-5)?
                .save_directory_invoke(table, ram, cpu, ordinal);
        }
        if route == 2 {
            return self
                .regular_files
                .as_mut()
                .ok_or(-5)?
                .regular_files_invoke(ram, table, cpu, ordinal);
        }
        let (_, rom_events) = self.owner.startup_objects(ram)?;
        self.rom_adapter.rom_kernel_invoke(
            self.image.image_rom_source(),
            table,
            &rom_events,
            ram,
            cpu,
            ordinal,
        )
    }

    // The creator finishes file requests before releasing the kernel owner.
    pub fn startup_files_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        let table = self.owner.startup_rom_table(ram, self.image)?;
        if let Some(regular) = self.regular_files.as_mut() {
            regular.regular_files_drain(ram, table)?;
        }
        if let Some(directories) = self.private_directories.as_mut() {
            directories.save_directories_drain(ram, table)?;
        }
        self.rom_adapter.rom_kernel_drain(ram, table)
    }
}

fn startup_open_route(
    ram: &memory::Memory,
    cpu: &registers::Registers,
    ordinal: u32,
    has_regular: bool,
) -> Result<u32, i32> {
    let attributes = ram.read_u32(cpu.esp.checked_add(12).ok_or(-1)?)?;
    let counted_path = kernel_file::rom_kernel_name(ram, attributes)?;
    let path = gamepkg::text_from_utf8(&counted_path)?;
    let partition = "\\Device\\Harddisk0\\partition1\\";
    if let Some(prefix) = path.get(..partition.len()) {
        if prefix.eq_ignore_ascii_case(partition) {
            if !has_regular {
                return Ok(1);
            }
            let options_index = if ordinal == 190 { 36 } else { 24 };
            let options = ram.read_u32(cpu.esp.checked_add(options_index).ok_or(-1)?)?;
            return Ok(if options & 1 != 0 { 1 } else { 2 });
        }
    }
    Ok(0)
}
