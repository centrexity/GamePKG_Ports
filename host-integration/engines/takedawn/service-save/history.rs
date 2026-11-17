// Port-owned, asset-free history export. Only shared capability I/O is used.
use crate::files;
use crate::gamepkg;
use crate::paths;
use crate::state;

pub struct Archived {
    grant: u64,
    folder: std::string::String,
    title: std::string::String,
    real: std::string::String,
    pending: std::string::String,
}
impl Archived {
    pub fn archive_root(&self) -> &str {
        &self.folder
    }
    pub fn valid_for(&self, grant: u64, real: &str, pending: &str) -> bool {
        self.grant == grant && self.real == real && self.pending == pending && !real.is_empty()
    }
    pub fn verify_pending(&self, grant: u64, real: &str, pending: &str) -> Result<(), i32> {
        if !self.valid_for(grant, real, pending) {
            return Err(-4);
        }
        if !archive_complete(grant, &self.folder, &self.title)? {
            return Err(-3);
        }
        let target = history_child(&self.folder, real)?;
        if !gamepkg::file_exists(grant, &target)? || !files::same(grant, pending, &target)? {
            return Err(-3);
        }
        Ok(())
    }
}
struct HistoryBudget {
    entries: u32,
    bytes: u64,
}
impl HistoryBudget {
    fn charge_entries(&mut self, count: usize) -> Result<(), i32> {
        if count > 10000 {
            return Err(-2);
        }
        self.entries = self.entries.checked_add(count as u32).ok_or(-2)?;
        if self.entries > 10000 {
            return Err(-2);
        }
        Ok(())
    }
    fn charge_file(&mut self, count: u64) -> Result<(), i32> {
        self.bytes = self.bytes.checked_add(count).ok_or(-2)?;
        if self.bytes > 1073741824 {
            return Err(-2);
        }
        Ok(())
    }
}
fn history_text(text: &str) -> std::string::String {
    let mut value = std::string::String::new();
    value.push_str(text);
    value
}
pub fn history_child(parent: &str, name: &str) -> Result<std::string::String, i32> {
    let mut value = history_text(parent);
    if !parent.is_empty() {
        value.push('/');
    }
    value.push_str(name);
    if value.len() > 4096 {
        return Err(-1);
    }
    Ok(value)
}
fn history_title_valid(title: &str) -> bool {
    if title.len() != 8 {
        return false;
    }
    for byte in title.bytes() {
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}
fn history_stamp_valid(stamp: &str) -> bool {
    if stamp.len() != 15 || stamp.as_bytes()[8] != b'-' {
        return false;
    }
    for (index, byte) in stamp.bytes().enumerate() {
        if index != 8 && !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}
pub fn directory_present(grant: u64, path: &str) -> Result<bool, i32> {
    if path.is_empty() {
        let _ = gamepkg::directory_list(grant, "")?;
        return Ok(true);
    }
    let mut parent = std::string::String::new();
    for component in paths::segments(path)? {
        let _ = gamepkg::text_component(component.as_bytes())?;
        let entries = gamepkg::directory_list(grant, &parent)?;
        let mut found = false;
        for entry in entries {
            if entry.name == component {
                if !entry.directory {
                    return Err(-4);
                }
                found = true;
                break;
            }
        }
        if !found {
            return Ok(false);
        }
        parent = history_child(&parent, component)?;
    }
    Ok(true)
}
fn history_ensure_dir(grant: u64, path: &str) -> Result<(), i32> {
    let mut parent = std::string::String::new();
    for component in paths::segments(path)? {
        let _ = gamepkg::text_component(component.as_bytes())?;
        parent = history_child(&parent, component)?;
        if !directory_present(grant, &parent)? {
            gamepkg::directory_create(grant, &parent)?;
        }
    }
    Ok(())
}
fn history_number(number: u32) -> std::string::String {
    let mut digits = [0u8; 10];
    let mut length = 0usize;
    let mut remaining = number;
    loop {
        digits[length] = (remaining % 10) as u8;
        length += 1;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    let mut text = std::string::String::new();
    for index in (0..length).rev() {
        text.push((b'0' + digits[index]) as char);
    }
    text
}
fn history_destination(grant: u64, stamp: &str) -> Result<std::string::String, i32> {
    history_ensure_dir(grant, "save-history")?;
    for number in 1u32..100u32 {
        let mut leaf = history_text(stamp);
        if number > 1 {
            leaf.push('-');
            leaf.push_str(&history_number(number));
        }
        let folder = history_child("save-history", &leaf)?;
        if directory_present(grant, &folder)? {
            continue;
        }
        // Exclusive directory creation, never reuse/overwrite a previous export.
        gamepkg::directory_create(grant, &folder)?;
        return Ok(folder);
    }
    Err(-2)
}
fn history_copy_one(
    grant: u64,
    source: &str,
    target: &str,
    budget: &mut HistoryBudget,
) -> Result<(), i32> {
    let handle = gamepkg::file_open(grant, false, source)?;
    let length = gamepkg::file_size(handle);
    let close_status = gamepkg::file_close(handle);
    let length = length?;
    close_status?;
    budget.charge_file(length)?;
    if files::copy_new(grant, source, target)? != length || !files::same(grant, source, target)? {
        return Err(-3);
    }
    Ok(())
}
fn history_copy_tree(
    grant: u64,
    source: &str,
    target: &str,
    depth: u32,
    budget: &mut HistoryBudget,
) -> Result<u32, i32> {
    if depth > 32 {
        return Err(-2);
    }
    history_ensure_dir(grant, target)?;
    let entries = gamepkg::directory_list(grant, source)?;
    budget.charge_entries(entries.len())?;
    let mut count = 0u32;
    for entry in entries {
        let from = history_child(source, &entry.name)?;
        let to = history_child(target, &entry.name)?;
        if entry.directory {
            count = count
                .checked_add(history_copy_tree(grant, &from, &to, depth + 1, budget)?)
                .ok_or(-2)?;
        } else if !state::hidden(&entry.name) {
            history_copy_one(grant, &from, &to, budget)?;
            count = count.checked_add(1).ok_or(-2)?;
        }
    }
    Ok(count)
}
fn history_remove_tree(
    grant: u64,
    path: &str,
    depth: u32,
    budget: &mut HistoryBudget,
) -> Result<(), i32> {
    if depth > 32 {
        return Err(-2);
    }
    let entries = gamepkg::directory_list(grant, path)?;
    budget.charge_entries(entries.len())?;
    for entry in entries {
        let child = history_child(path, &entry.name)?;
        if entry.directory {
            history_remove_tree(grant, &child, depth + 1, budget)?;
        } else {
            gamepkg::file_remove(grant, &child)?;
        }
    }
    gamepkg::directory_remove(grant, path)
}
fn history_marker(
    grant: u64,
    folder: &str,
    title: &str,
    stamp: &str,
    note: &str,
) -> Result<(), i32> {
    let target = history_child(folder, "save-export.txt")?;
    let temporary = history_child(folder, "save-export.txt.~new")?;
    let mut text = history_text("title ");
    text.push_str(title);
    text.push_str("\nmade ");
    text.push_str(stamp);
    text.push('\n');
    text.push_str(note);
    if note.as_bytes().last() != Some(&b'\n') {
        text.push('\n');
    }
    let handle = gamepkg::file_create_new(grant, &temporary)?;
    let written = gamepkg::file_write(handle, 0, text.as_bytes());
    let close_status = gamepkg::file_close(handle);
    written?;
    close_status?;
    match files::publish(grant, &temporary, &target)? {
        files::Publication::Durable {} => Ok(()),
        files::Publication::CommittedNotDurable {} => Err(-6),
    }
}
fn history_build(
    grant: u64,
    title: &str,
    stamp: &str,
    note: &str,
    folder: &str,
    real: &str,
    pending: &str,
) -> Result<(), i32> {
    let mut budget = HistoryBudget {
        entries: 0,
        bytes: 0,
    };
    let mut userdata = 0u32;
    for partition in ["UDATA", "TDATA"] {
        let relative = history_child(partition, title)?;
        if !directory_present(grant, &relative)? {
            continue;
        }
        let target = history_child(folder, &relative)?;
        let count = history_copy_tree(grant, &relative, &target, 0, &mut budget)?;
        if partition == "UDATA" {
            userdata = count;
        }
    }
    if userdata == 0 {
        return Err(-3);
    }
    if !real.is_empty() {
        let target = history_child(folder, real)?;
        if gamepkg::file_exists(grant, &target)? {
            gamepkg::file_remove(grant, &target)?;
        }
        history_copy_one(grant, pending, &target, &mut budget)?;
    }
    history_marker(grant, folder, title, stamp, note)
}
pub fn archive_pending(
    grant: u64,
    title: &str,
    stamp: &str,
    real: &str,
    pending: &str,
) -> Result<Archived, i32> {
    if !history_title_valid(title) || !history_stamp_valid(stamp) {
        return Err(-1);
    }
    let mut expected = history_text(real);
    expected.push_str(".~pend");
    if pending != expected || state::hidden(real) {
        return Err(-1);
    }
    let mut scoped = false;
    for partition in ["UDATA", "TDATA"] {
        let mut prefix = history_child(partition, title)?;
        prefix.push('/');
        if real.starts_with(&prefix) && real.len() > prefix.len() {
            scoped = true;
        }
    }
    if !scoped {
        return Err(-4);
    }
    let folder = history_destination(grant, stamp)?;
    if let Err(error) = history_build(
        grant,
        title,
        stamp,
        "kept at the start: a pending save is in this complete title snapshot",
        &folder,
        real,
        pending,
    ) {
        let _ = history_remove_tree(
            grant,
            &folder,
            0,
            &mut HistoryBudget {
                entries: 0,
                bytes: 0,
            },
        );
        return Err(error);
    }
    Ok(Archived {
        grant,
        folder,
        title: history_text(title),
        real: history_text(real),
        pending: history_text(pending),
    })
}
pub fn archive_before_replace(grant: u64, title: &str, stamp: &str) -> Result<Archived, i32> {
    if !history_title_valid(title) || !history_stamp_valid(stamp) {
        return Err(-1);
    }
    let folder = history_destination(grant, stamp)?;
    if let Err(error) = history_build(
        grant,
        title,
        stamp,
        "kept before the game saved over it",
        &folder,
        "",
        "",
    ) {
        let _ = history_remove_tree(
            grant,
            &folder,
            0,
            &mut HistoryBudget {
                entries: 0,
                bytes: 0,
            },
        );
        return Err(error);
    }
    Ok(Archived {
        grant,
        folder,
        title: history_text(title),
        real: std::string::String::new(),
        pending: std::string::String::new(),
    })
}
// Completion marker is mandatory for migrated history listings. A process that
// ends mid-copy must not leave a partially copied tree advertised as restorable.
pub fn archive_complete(grant: u64, folder: &str, title: &str) -> Result<bool, i32> {
    if !history_title_valid(title) {
        return Err(-1);
    }
    let marker = history_child(folder, "save-export.txt")?;
    if !gamepkg::file_exists(grant, &marker)? {
        return Ok(false);
    }
    let bytes = files::read_small(grant, &marker, 16384)?;
    if bytes.len() < 15 || &bytes[..6] != b"title " {
        return Ok(false);
    }
    for index in 0..8 {
        if bytes[6 + index].to_ascii_lowercase() != title.as_bytes()[index].to_ascii_lowercase() {
            return Ok(false);
        }
    }
    if bytes[14] != b'\n' && bytes[14] != b'\r' {
        return Ok(false);
    }
    let userdata = history_child(&history_child(folder, "UDATA")?, title)?;
    if !directory_present(grant, &userdata)? {
        return Ok(false);
    }
    let mut stack = std::vec::Vec::new();
    stack.push((userdata, 0u32));
    let mut budget = HistoryBudget {
        entries: 0,
        bytes: 0,
    };
    while let Some((path, depth)) = stack.pop() {
        if depth > 32 {
            return Err(-2);
        }
        let entries = gamepkg::directory_list(grant, &path)?;
        budget.charge_entries(entries.len())?;
        for entry in entries {
            if entry.directory {
                stack.push((history_child(&path, &entry.name)?, depth + 1));
            } else if !state::hidden(&entry.name) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
