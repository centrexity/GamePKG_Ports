// Maintainer harness. Only files.rs belongs to game source.
#[path = "files.rs"]
mod production;
use host::port_host::{Backend, Frame, Result, Session, WindowCommand};
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd};
use std::os::unix::net::UnixStream;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Output;
impl Backend for Output {
    fn window(&mut self, _: WindowCommand) -> Result<()> {
        unreachable!()
    }
    fn present(&mut self, _: Frame) -> Result<()> {
        unreachable!()
    }
    fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
        unreachable!()
    }
    fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
        unreachable!()
    }
}
struct Harness {
    thread: Option<std::thread::JoinHandle<()>>,
    publications: Arc<AtomicUsize>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        gamepkg::disconnect();
        self.thread.take().unwrap().join().unwrap();
    }
}
fn connect(
    root: &std::path::Path,
    quota: u64,
    short_reads: bool,
    uncertain: bool,
) -> (u64, Harness) {
    let app = std::path::PathBuf::from(std::env::var_os("GAMEPKG_APP").unwrap());
    let mut manifest = host::manifest::Manifest::parse(
        &std::fs::read(app.join("examples/minimal/manifest.toml")).unwrap(),
    )
    .unwrap();
    manifest.permissions.save = true;
    let mut session = Session::new(manifest.permissions, Output);
    let grant = session
        .grant_save_quota(
            cap_std::fs::Dir::open_ambient_dir(root, cap_std::ambient_authority()).unwrap(),
            quota,
        )
        .unwrap();
    let (client, mut server) = UnixStream::pair().unwrap();
    let input = unsafe { std::fs::File::from_raw_fd(client.try_clone().unwrap().into_raw_fd()) };
    let output = unsafe { std::fs::File::from_raw_fd(client.into_raw_fd()) };
    gamepkg::connect(input, output).unwrap();
    let publications = Arc::new(AtomicUsize::new(0));
    let count = publications.clone();
    let thread = std::thread::spawn(move || loop {
        let mut header = [0; 16];
        match server.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => panic!("{error}"),
        }
        assert_eq!(&header[..4], b"GPH1");
        let op = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let length = u32::from_le_bytes(header[8..12].try_into().unwrap());
        let cap = u32::from_le_bytes(header[12..].try_into().unwrap());
        let mut request = vec![0; length as usize];
        server.read_exact(&mut request).unwrap();
        if short_reads && op == 2 {
            let count = u32::from_le_bytes(request[16..20].try_into().unwrap()).min(1111);
            request[16..20].copy_from_slice(&count.to_le_bytes());
        }
        if op == 14 {
            count.fetch_add(1, Ordering::SeqCst);
        }
        // Synthetic post-publication status injection; no actual durability
        // failure is caused, and this mode is not proof of a published file.
        let result = if uncertain && op == 14 {
            Err(host::port_host::Error::CommittedNotDurable)
        } else {
            session.call(op, &request, cap)
        };
        let (status, reply) = match result {
            Ok(reply) => (0i32, reply),
            Err(error) => (error as i32, Vec::new()),
        };
        server.write_all(&status.to_le_bytes()).unwrap();
        server
            .write_all(&(reply.len() as u32).to_le_bytes())
            .unwrap();
        server.write_all(&reply).unwrap();
    });
    (
        grant,
        Harness {
            thread: Some(thread),
            publications,
        },
    )
}

#[test]
fn whole_copy_short_reads_comparison_flush_and_atomic_publish_use_real_services() {
    let root = tempfile::tempdir().unwrap();
    let bytes: Vec<_> = (0..131_089).map(|n| (n % 251) as u8).collect();
    std::fs::write(root.path().join("profile"), &bytes).unwrap();
    let (grant, harness) = connect(root.path(), 1024 * 1024, true, false);
    assert_eq!(
        production::copy_shadow(grant, "profile"),
        Ok(bytes.len() as u64)
    );
    assert_eq!(
        std::fs::read(root.path().join("profile.~new")).unwrap(),
        bytes
    );
    assert_eq!(production::same(grant, "profile", "profile.~new"), Ok(true));
    let writer = gamepkg::file_open(grant, true, "profile.~new").unwrap();
    let retained = production::retain_for_flush(writer).unwrap();
    gamepkg::file_close(writer).unwrap();
    gamepkg::file_write(retained, 70_000, b"changed").unwrap();
    production::sync_close(retained).unwrap();
    assert_eq!(gamepkg::file_size(retained), Err(-4));
    assert_eq!(
        production::same(grant, "profile", "profile.~new"),
        Ok(false)
    );
    for _ in 0..140 {
        assert_eq!(production::same(grant, "profile", "profile"), Ok(true));
    }
    let modified = production::modified(grant, "profile").unwrap();
    assert!(modified.1 < 1_000_000_000);
    assert!(matches!(
        production::publish(grant, "profile.~new", "profile"),
        Ok(production::Publication::Durable {})
    ));
    assert!(!root.path().join("profile.~new").exists());
    assert_eq!(
        &std::fs::read(root.path().join("profile")).unwrap()[70_000..70_007],
        b"changed"
    );
    assert_eq!(harness.publications.load(Ordering::SeqCst), 1);
    assert!(production::copy_shadow(grant, "../outside").is_err());
    assert!(production::modified(grant, "missing").is_err());
    assert!(production::same(grant, "profile", "missing").is_err());
}

#[test]
fn failed_copy_removes_only_partial_shadow_and_preserves_the_real_save() {
    let root = tempfile::tempdir().unwrap();
    let bytes = vec![123u8; 131_089];
    std::fs::write(root.path().join("profile"), &bytes).unwrap();
    let (grant, _harness) = connect(root.path(), 210_000, false, false);
    for _ in 0..140 {
        assert_eq!(production::copy_shadow(grant, "profile"), Err(-2));
        assert!(!root.path().join("profile.~new").exists());
        assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), bytes);
    }
}

#[test]
fn published_without_durability_is_explicit_and_never_blindly_retried() {
    let root = tempfile::tempdir().unwrap();
    let (grant, harness) = connect(root.path(), 1024, false, true);
    assert!(matches!(
        production::publish(grant, "profile.~new", "profile"),
        Ok(production::Publication::CommittedNotDurable {})
    ));
    assert_eq!(harness.publications.load(Ordering::SeqCst), 1);
}
