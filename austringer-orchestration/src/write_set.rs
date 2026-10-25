//! The disjoint-write-set contract for parallel children and the
//! FileTouch collision alerts (jcode SS32): every write/edit publishes a
//! `FileTouch { path, op, intent }` and the registry alerts BOTH sides
//! when siblings touch overlapping files.

/// A child's declared write scope (globs; empty = read-only).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteSet {
    /// Declared globs.
    pub globs: Vec<String>,
}

/// Literal (wildcard-free) head of a glob, without a trailing separator.
fn literal_head(glob: &str) -> String {
    let cut = glob.find(['*', '?', '[']).unwrap_or(glob.len());
    glob[..cut].trim_end_matches('/').to_string()
}

/// Component-wise path-prefix relation on `/`-separated heads.
fn head_covers(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return true;
    }
    let mut left = a.split('/');
    let mut right = b.split('/');
    loop {
        match (left.next(), right.next()) {
            (None, _) | (_, None) => return true,
            (Some(x), Some(y)) if x == y => continue,
            (Some(_), Some(_)) => return false,
        }
    }
}

impl WriteSet {
    /// Parses a comma-separated glob list (empty input = empty set).
    pub fn parse(input: &str) -> Self {
        Self {
            globs: input
                .split(',')
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .map(String::from)
                .collect(),
        }
    }

    /// Read-only?
    pub fn is_empty(&self) -> bool {
        self.globs.is_empty()
    }

    /// Conservative overlap detection (fail closed: literal heads that are
    /// component-wise prefix-related count as overlapping even when the
    /// wildcard remainders might be disjoint). Returns the overlapping
    /// globs from `self`.
    pub fn overlap_with(&self, other: &WriteSet) -> Vec<String> {
        self.globs
            .iter()
            .filter(|mine| {
                other.globs.iter().any(|theirs| {
                    let (a, b) = (literal_head(mine), literal_head(theirs));
                    head_covers(&a, &b) || head_covers(&b, &a)
                })
            })
            .cloned()
            .collect()
    }
}

/// One file-touch operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchOp {
    /// New content.
    Write,
    /// Targeted change.
    Edit,
    /// New file.
    Create,
    /// Removal.
    Delete,
}

impl TouchOp {
    /// Stable wire name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Edit => "edit",
            Self::Create => "create",
            Self::Delete => "delete",
        }
    }
}

/// One published touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTouch {
    /// Touched path.
    pub path: String,
    /// Operation.
    pub op: TouchOp,
    /// Why the child touched it.
    pub intent: String,
    /// Touching child id.
    pub child: String,
}

/// A sibling collision on one path, naming BOTH sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// The contested path.
    pub path: String,
    /// First (registered) toucher.
    pub a: String,
    /// Second toucher.
    pub b: String,
    /// First toucher's operation.
    pub a_op: TouchOp,
    /// Second toucher's operation.
    pub b_op: TouchOp,
}

/// Registry of active touches. The FIRST toucher of a path stays its
/// registered owner until retired; later sibling touchers collide (and
/// are alerted), which is deliberately conservative.
#[derive(Debug, Default)]
pub struct TouchRegistry {
    active: std::collections::BTreeMap<String, FileTouch>,
}

impl TouchRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a touch; returns a collision when another active child
    /// already owns the path.
    pub fn record(&mut self, touch: FileTouch) -> Option<Collision> {
        let collision = self.active.get(&touch.path).and_then(|owner| {
            (owner.child != touch.child).then(|| Collision {
                path: touch.path.clone(),
                a: owner.child.clone(),
                b: touch.child.clone(),
                a_op: owner.op,
                b_op: touch.op,
            })
        });
        if !collision.is_some() {
            self.active.insert(touch.path.clone(), touch);
        }
        collision
    }

    /// Retires a child: its owned paths stop colliding.
    pub fn retire(&mut self, child: &str) {
        self.active.retain(|_, touch| touch.child != child);
    }
}
