//! Shared read-only capture registry and generated-output authorization.
use crate::{Error, Result};
use std::path::{Component, Path, PathBuf};

/// Machine-local egress policy; unclassified sources are denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Egress {
    /// No provider export.
    Deny,
    /// Human-authorized provider export (transport authorization still required).
    Allow,
}
/// Normalized, independently browsable read-only root.
#[derive(Debug, Clone)]
pub struct Root {
    /// Stable local display name.
    pub name: String,
    /// Canonical filesystem location.
    pub path: PathBuf,
    /// Machine-local policy.
    pub egress: Egress,
}
/// One policy used by both local transports. Targets authorize aliases only.
#[derive(Debug, Clone)]
pub struct RootPolicy {
    /// Read-only registry.
    pub roots: Vec<Root>,
    output: Option<PathBuf>,
    follow: bool,
    targets: Vec<PathBuf>,
}
impl RootPolicy {
    /// Normalize human-supplied startup settings. Output must be outside inputs/targets.
    pub fn new(
        roots: &[PathBuf],
        output: Option<&Path>,
        follow: bool,
        targets: &[PathBuf],
    ) -> Result<Self> {
        let mut registry: Vec<Root> = Vec::new();
        for given in roots {
            let path = crate::paths::canonicalize(given)
                .map_err(|e| Error::Config(format!("root {}: {e}", given.display())))?;
            if !path.is_dir() || registry.iter().any(|r| r.path == path) {
                return Err(Error::Config("roots must be distinct directories".into()));
            }
            let base = path
                .file_name()
                .map_or_else(|| "root".into(), |s| s.to_string_lossy().into_owned());
            let mut name = base.clone();
            let mut n = 2;
            while registry.iter().any(|r| r.name == name) {
                name = format!("{base}-{n}");
                n += 1;
            }
            registry.push(Root {
                name,
                path,
                egress: Egress::Deny,
            });
        }
        let targets = targets
            .iter()
            .map(|p| crate::paths::canonicalize(p).map_err(|e| Error::Config(e.to_string())))
            .collect::<Result<Vec<_>>>()?;
        Self::from_normalized(registry, output, follow, targets)
    }
    /// Construct from storage probes already bounded by the serving transport.
    pub fn from_normalized(
        roots: Vec<Root>,
        output: Option<&Path>,
        follow: bool,
        targets: Vec<PathBuf>,
    ) -> Result<Self> {
        if roots.is_empty() {
            return Err(Error::Config("at least one --root is required".into()));
        }
        let output = output.map(crate::run::normalise_path);
        if output.as_ref().is_some_and(|p| {
            roots
                .iter()
                .any(|r| p.starts_with(&r.path) || r.path.starts_with(p))
                || targets.iter().any(|t| p.starts_with(t) || t.starts_with(p))
        }) {
            return Err(Error::Config(
                "--out-root must be separate from read-only capture roots and symlink targets"
                    .into(),
            ));
        }
        Ok(Self {
            roots,
            output,
            follow,
            targets,
        })
    }
    /// The innermost lexical registry root. Storage targets are not roots.
    pub fn root_of(&self, path: &Path) -> Option<&Root> {
        self.roots
            .iter()
            .filter(|r| path.starts_with(&r.path))
            .max_by_key(|r| r.path.components().count())
    }
    /// Canonical target authorization after a route has been located under a root.
    pub fn allows(&self, home: &Path, canonical: &Path) -> bool {
        self.roots
            .iter()
            .any(|r| (r.path == home || self.follow) && canonical.starts_with(&r.path))
            || self.targets.iter().any(|t| canonical.starts_with(t))
    }
    // Canonicalize through the first registry/output boundary, retaining the
    // suffix as a lexical route. Resolving the whole route would turn a symlink
    // into another root into direct access, bypassing `follow` and alias rules.
    fn route(&self, path: &Path) -> PathBuf {
        let mut route = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    route.pop();
                }
                Component::CurDir => {}
                other => route.push(other.as_os_str()),
            }
        }
        let mut prefix = PathBuf::new();
        let mut components = route.components();
        while let Some(component) = components.next() {
            prefix.push(component.as_os_str());
            // Outputs may not exist yet; this uses paths::canonicalize on the
            // longest existing prefix, with the same policy as startup roots.
            let canonical = crate::run::normalise_path(&prefix);
            if self.root_of(&canonical).is_some()
                || self
                    .output
                    .as_ref()
                    .is_some_and(|out| canonical.starts_with(out))
            {
                return canonical.join(components.as_path());
            }
        }
        route
    }
    /// Resolve an input, including a generated artifact, while retaining the alias route.
    pub fn read(&self, given: &Path) -> Result<PathBuf> {
        let given = crate::paths::native(given);
        let joined = if given.is_absolute() {
            given.to_path_buf()
        } else {
            self.roots[0].path.join(given.as_ref())
        };
        let canonical = crate::paths::canonicalize(&joined)
            .map_err(|e| Error::Config(format!("resolving {}: {e}", joined.display())))?;
        let route = self.route(&joined);
        let allowed = self
            .root_of(&route)
            .is_some_and(|r| self.allows(&r.path, &canonical))
            || self
                .output
                .as_ref()
                .is_some_and(|out| route.starts_with(out) && canonical.starts_with(out));
        if !allowed {
            return Err(Error::Config("input escapes registered roots".into()));
        }
        Ok(joined)
    }
    /// Resolve a new output using its existing parent, never a capture permission.
    pub fn write(&self, given: &Path) -> Result<PathBuf> {
        let given = crate::paths::native(given);
        let out = self
            .output
            .as_ref()
            .ok_or_else(|| Error::Config("generated artifacts require --out-root".into()))?;
        if given
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            return Err(Error::Config("output traversal is forbidden".into()));
        }
        let path = if given.is_absolute() {
            given.to_path_buf()
        } else {
            out.join(given.as_ref())
        };
        let canonical = crate::run::normalise_path(&path);
        if !self.route(&path).starts_with(out)
            || !canonical.starts_with(out)
            || self.roots.iter().any(|r| canonical.starts_with(&r.path))
            || self.targets.iter().any(|t| canonical.starts_with(t))
        {
            return Err(Error::Config("output escapes --out-root".into()));
        }
        Ok(canonical)
    }
    /// Whether a local generated directory stays outside capture storage.
    pub fn allows_output(&self, path: &Path) -> bool {
        !self.roots.iter().any(|r| path.starts_with(&r.path))
            && !self.targets.iter().any(|t| path.starts_with(t))
    }
    /// Denial wins for overlapping registered sources. Unknown provenance is denied.
    pub fn egress(&self, path: &Path) -> Egress {
        let matches: Vec<_> = self
            .roots
            .iter()
            .filter(|r| path.starts_with(&r.path))
            .collect();
        if matches.is_empty() || matches.iter().any(|r| r.egress == Egress::Deny) {
            Egress::Deny
        } else {
            Egress::Allow
        }
    }
}
