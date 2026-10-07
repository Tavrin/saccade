//! Capability-relative I/O for synchronous root-policy operations.
//!
//! The thread-bound scope propagates authority through evidence decoding and
//! hashing. Callers must not carry it across async tasks or thread spawns.
use super::RootPolicy;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use std::{
    cell::RefCell,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
/// Internal report-index coordination file; never an output artifact.
pub const INDEX_LOCK_FILE: &str = ".saccade-index.lock";
thread_local! { static POLICY: RefCell<Option<RootPolicy>> = const { RefCell::new(None) }; }
/// Restores the previous thread policy on return or panic; cannot cross threads.
pub struct Scope(
    Option<RootPolicy>,
    std::marker::PhantomData<std::rc::Rc<()>>,
);
impl Drop for Scope {
    fn drop(&mut self) {
        POLICY.with(|p| *p.borrow_mut() = self.0.take());
    }
}
/// Bind evidence/artifact I/O until the returned guard drops.
pub fn scope(policy: &RootPolicy) -> Scope {
    Scope(
        POLICY.with(|p| p.replace(Some(policy.clone()))),
        std::marker::PhantomData,
    )
}
fn denied(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, e.to_string())
}
// Acquire canonical startup directories from the volume root, refusing every
// symlink component. Authorization must not race an ambient root-handle open.
pub(super) fn pin_directory(path: &Path, create: bool) -> io::Result<Dir> {
    pin_directory_after_check(path, create, || {})
}
fn pin_directory_after_check(path: &Path, create: bool, checked: impl FnOnce()) -> io::Result<Dir> {
    use std::path::Component;
    if !path.is_absolute() {
        return Err(denied("policy directory must be absolute"));
    }
    let mut anchor = PathBuf::new();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => anchor.push(component.as_os_str()),
            Component::Normal(name) => names.push(name),
            _ => return Err(denied("policy directory must be normalized")),
        }
    }
    let mut dir = Dir::open_ambient_dir(anchor, cap_std::ambient_authority())?;
    checked();
    for name in names {
        if create {
            match dir.create_dir(name) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        dir = dir.open_dir_nofollow(name)?;
    }
    Ok(dir)
}
impl RootPolicy {
    fn read_location(&self, path: &Path) -> io::Result<(&Dir, PathBuf)> {
        let (_, canonical) = self.resolve_read(path).map_err(denied)?;
        self.handles
            .iter()
            .filter_map(|(root, dir)| {
                canonical
                    .strip_prefix(root)
                    .ok()
                    .map(|p| (dir.as_ref(), p.to_owned()))
            })
            .min_by_key(|(_, p)| p.components().count())
            .ok_or_else(|| denied("input escapes registered roots"))
    }
    /// Open beneath a pinned authorized directory, with no ambient reopen.
    pub fn open_file(&self, path: &Path) -> io::Result<std::fs::File> {
        self.open_after_check(path, || {})
    }
    fn open_after_check(&self, path: &Path, checked: impl FnOnce()) -> io::Result<std::fs::File> {
        let (dir, relative) = self.read_location(path)?;
        checked();
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = dir.open_with(relative, &options)?.into_std();
        if !file.metadata()?.is_file() {
            return Err(denied("input must be a regular file"));
        }
        Ok(file)
    }
    /// Replace an output inode via an exclusive same-directory temporary file.
    pub fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.write_after_check(path, bytes, || {})
    }
    fn write_after_check(
        &self,
        path: &Path,
        bytes: &[u8],
        checked: impl FnOnce(),
    ) -> io::Result<()> {
        let (parent, name) = self.output_parent(path, checked)?;
        if parent
            .symlink_metadata(&name)
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(denied("refusing output symlink"));
        }
        atomic_replace(&parent, &name, bytes)
    }
    fn output_parent(&self, path: &Path, checked: impl FnOnce()) -> io::Result<(Dir, PathBuf)> {
        let path = self.write(path).map_err(denied)?;
        let root = self
            .output
            .as_ref()
            .ok_or_else(|| denied("missing output root"))?;
        let dir = self
            .handles
            .iter()
            .find(|(p, _)| p == root)
            .map(|(_, d)| d)
            .ok_or_else(|| denied("output directory unavailable"))?;
        let relative = path.strip_prefix(root).map_err(denied)?;
        checked();
        let parent = relative
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        dir.create_dir_all(parent)?;
        Ok((
            dir.open_dir(parent)?,
            PathBuf::from(
                relative
                    .file_name()
                    .ok_or_else(|| denied("output requires a filename"))?,
            ),
        ))
    }
}
static NEXT: AtomicU64 = AtomicU64::new(0);
fn atomic_replace(parent: &Dir, name: &Path, bytes: &[u8]) -> io::Result<()> {
    // Exclusive creation prevents a guessed name from granting existing-inode
    // access. Rename replaces links without opening or truncating their targets.
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let (temp, mut file) = (0..128)
        .find_map(|_| {
            let temp = format!(
                ".saccade-tmp-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            match parent.open_with(&temp, &options) {
                Ok(f) => Some(Ok((temp, f))),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => None,
                Err(e) => Some(Err(e)),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "temporary name quota exhausted",
            ))
        })?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        parent.rename(&temp, parent, name)
    })();
    if result.is_err() {
        let _ = parent.remove_file(&temp);
    }
    result
}
/// Open using the active policy, or ordinary local authority outside a scope.
pub fn open_file(path: impl AsRef<Path>) -> io::Result<std::fs::File> {
    POLICY.with(|p| match p.borrow().as_ref() {
        Some(policy) => policy.open_file(path.as_ref()),
        None => std::fs::File::open(crate::paths::native(path.as_ref())),
    })
}
/// Read evidence through the active pinned directories.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    open_file(path)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}
/// Decode UTF-8 after a capability-relative read.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
/// Atomically replace an artifact; never truncate an existing inode.
pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let path = path.as_ref();
    POLICY.with(|p| match p.borrow().as_ref() {
        Some(policy) => policy.write_file(path, bytes.as_ref()),
        None => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            std::fs::create_dir_all(parent)?;
            if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(denied("refusing output symlink"));
            }
            let mut temp = tempfile::NamedTempFile::new_in(parent)?;
            temp.write_all(bytes.as_ref())?;
            temp.as_file().sync_all()?;
            temp.persist(path).map_err(|e| e.error)?;
            Ok(())
        }
    })
}
/// Append an index row under a stable directory-relative lock, replacing the
/// index atomically so a hardlinked index cannot modify read-only storage.
pub fn append(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (parent, name) = POLICY.with(|p| match p.borrow().as_ref() {
        Some(policy) => policy.output_parent(path, || {}),
        None => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            std::fs::create_dir_all(parent)?;
            Ok((
                Dir::open_ambient_dir(parent, cap_std::ambient_authority())?,
                PathBuf::from(
                    path.file_name()
                        .ok_or_else(|| denied("index requires a filename"))?,
                ),
            ))
        }
    })?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let lock = parent.open_with(INDEX_LOCK_FILE, &options)?.into_std();
    if !lock.metadata()?.is_file() {
        return Err(denied("index lock must be a regular file"));
    }
    lock.lock()?;
    let mut read_options = OpenOptions::new();
    read_options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        read_options.custom_flags(libc::O_NONBLOCK);
    }
    let mut previous = Vec::new();
    match parent.open_with(&name, &read_options) {
        Ok(mut file) => {
            if !file.metadata()?.is_file() {
                return Err(denied("index must be a regular file"));
            }
            file.read_to_end(&mut previous)?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    previous.extend_from_slice(bytes);
    atomic_replace(&parent, &name, &previous)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, RootPolicy, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input");
        let out = temp.path().join("out");
        std::fs::create_dir(&input).unwrap();
        std::fs::create_dir(&out).unwrap();
        let policy = RootPolicy::new(std::slice::from_ref(&input), Some(&out), false, &[]).unwrap();
        (temp, policy, input, out)
    }
    #[cfg(unix)]
    #[test]
    fn checked_read_and_write_replacement_races_are_confined() {
        use std::os::unix::fs::symlink;
        let (temp, policy, input, out) = fixture();
        let foreign = temp.path().join("foreign");
        std::fs::create_dir(&foreign).unwrap();
        std::fs::write(foreign.join("file"), b"secret").unwrap();
        let read = input.join("file");
        std::fs::write(&read, b"allowed").unwrap();
        assert!(
            policy
                .open_after_check(&read, || {
                    std::fs::remove_file(&read).unwrap();
                    symlink(foreign.join("file"), &read).unwrap();
                })
                .is_err()
        );
        let child = out.join("child");
        std::fs::create_dir(&child).unwrap();
        assert!(
            policy
                .write_after_check(&child.join("file"), b"overwrite", || {
                    std::fs::remove_dir(&child).unwrap();
                    symlink(&foreign, &child).unwrap();
                })
                .is_err()
        );
        assert_eq!(std::fs::read(foreign.join("file")).unwrap(), b"secret");
        let input_child = input.join("child");
        std::fs::create_dir(&input_child).unwrap();
        std::fs::write(input_child.join("file"), b"allowed").unwrap();
        assert!(
            policy
                .open_after_check(&input_child.join("file"), || {
                    std::fs::rename(&input_child, input.join("moved-child")).unwrap();
                    symlink(&foreign, &input_child).unwrap();
                })
                .is_err()
        );
        let final_output = out.join("final");
        assert!(
            policy
                .write_after_check(&final_output, b"overwrite", || {
                    symlink(foreign.join("file"), &final_output).unwrap();
                })
                .is_err()
        );
        assert_eq!(std::fs::read(foreign.join("file")).unwrap(), b"secret");
        // Replacing the root pathname cannot replace the pinned root handle.
        std::fs::remove_file(&read).unwrap();
        std::fs::write(&read, b"allowed").unwrap();
        let mut opened = policy
            .open_after_check(&read, || {
                std::fs::rename(&input, temp.path().join("moved")).unwrap();
                symlink(&foreign, &input).unwrap();
            })
            .unwrap();
        let mut bytes = Vec::new();
        opened.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"allowed");
        policy
            .write_after_check(&out.join("file"), b"generated", || {
                std::fs::rename(&out, temp.path().join("moved-out")).unwrap();
                symlink(&foreign, &out).unwrap();
            })
            .unwrap();
        assert_eq!(
            std::fs::read(temp.path().join("moved-out/file")).unwrap(),
            b"generated"
        );
        assert_eq!(std::fs::read(foreign.join("file")).unwrap(), b"secret");
    }
    #[cfg(unix)]
    #[test]
    fn startup_directory_replacement_cannot_grant_outside_handle() {
        let (temp, _policy, input, _out) = fixture();
        let foreign = temp.path().join("foreign");
        std::fs::create_dir(&foreign).unwrap();
        assert!(
            pin_directory_after_check(&input, false, || {
                std::fs::rename(&input, temp.path().join("moved")).unwrap();
                std::os::unix::fs::symlink(&foreign, &input).unwrap();
            })
            .is_err()
        );
        assert!(pin_directory(&input.join("new"), true).is_err());
        assert!(!foreign.join("new").exists());
    }
    #[test]
    fn output_hardlink_does_not_modify_read_only_input_inode() {
        let (_temp, policy, input, out) = fixture();
        let baseline = input.join("baseline");
        std::fs::write(&baseline, b"baseline").unwrap();
        let output = out.join("requests.json");
        std::fs::hard_link(&baseline, &output).unwrap();
        policy.write_file(&output, b"new output").unwrap();
        assert_eq!(std::fs::read(baseline).unwrap(), b"baseline");
        assert_eq!(std::fs::read(output).unwrap(), b"new output");
    }
    #[cfg(unix)]
    #[test]
    fn evidence_hashing_preserves_alias_authorization() {
        use crate::evidence::{canonical::Digest, case::ArtifactRef};
        let (temp, _policy, input, out) = fixture();
        let other = temp.path().join("other");
        std::fs::create_dir(&other).unwrap();
        std::fs::write(other.join("file"), b"other root").unwrap();
        std::os::unix::fs::symlink(other.join("file"), input.join("alias")).unwrap();
        let policy = RootPolicy::new(&[input.clone(), other], Some(&out), false, &[]).unwrap();
        let _scope = scope(&policy);
        let reference = ArtifactRef {
            path: "alias".into(),
            sha256: Digest::of_bytes(b"other root"),
        };
        assert!(reference.verify(&input.join("case.json")).is_err());
        assert!(
            ArtifactRef::from_file(&input.join("alias"), &input.join("case.json"), false).is_err()
        );
    }
    #[test]
    fn hardlinked_index_is_replaced_without_truncating_input() {
        let (_temp, policy, input, out) = fixture();
        let source = input.join("source");
        std::fs::write(&source, b"original\n").unwrap();
        let index = out.join("index.jsonl");
        std::fs::hard_link(&source, &index).unwrap();
        let _scope = scope(&policy);
        append(&index, b"new row\n").unwrap();
        assert_eq!(std::fs::read(source).unwrap(), b"original\n");
        assert_eq!(std::fs::read(index).unwrap(), b"original\nnew row\n");
    }
    #[test]
    fn scope_restores_policy_after_early_return() {
        let (_temp, policy, input, _out) = fixture();
        let file = input.join("file");
        std::fs::write(&file, b"allowed").unwrap();
        {
            let _scope = scope(&policy);
            assert_eq!(read(Path::new("file")).unwrap(), b"allowed");
        }
        assert_eq!(read(&file).unwrap(), b"allowed");
    }
}
