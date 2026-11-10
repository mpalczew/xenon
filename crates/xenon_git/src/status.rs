#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DirtState {
    #[default]
    Clean,
    IndexOnly,
    Dirty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflict,
}

impl ChangeKind {
    fn of(letter: char) -> Option<Self> {
        match letter {
            'M' | 'T' => Some(Self::Modified),
            'A' => Some(Self::Added),
            'D' => Some(Self::Deleted),
            'R' | 'C' => Some(Self::Renamed),
            '?' => Some(Self::Untracked),
            'U' => Some(Self::Conflict),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitFile {
    pub index: char,
    pub worktree: char,
    pub path: String,
    pub from: Option<String>,
    pub added: u32,
    pub removed: u32,
}

impl GitFile {
    pub fn is_untracked(&self) -> bool {
        self.index == '?'
    }

    pub(crate) fn is_conflict(&self) -> bool {
        matches!(
            (self.index, self.worktree),
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D')
        )
    }

    pub fn kind(&self) -> ChangeKind {
        if self.is_conflict() {
            return ChangeKind::Conflict;
        }
        if self.is_untracked() {
            return ChangeKind::Untracked;
        }
        let (index, tree) = (ChangeKind::of(self.index), ChangeKind::of(self.worktree));
        [ChangeKind::Added, ChangeKind::Deleted, ChangeKind::Renamed]
            .into_iter()
            .find(|k| index == Some(*k) || tree == Some(*k))
            .or(index)
            .or(tree)
            .unwrap_or(ChangeKind::Modified)
    }

    pub fn badges(&self) -> Vec<(char, ChangeKind)> {
        if self.is_untracked() {
            return vec![('?', ChangeKind::Untracked)];
        }
        [self.index, self.worktree]
            .into_iter()
            .filter_map(|c| ChangeKind::of(c).map(|k| (c, k)))
            .collect()
    }

    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    pub(crate) fn dir_tail(&self) -> String {
        const BUDGET: usize = 10;
        let parts: Vec<&str> = self.dir().split('/').filter(|p| !p.is_empty()).collect();
        let tail = &parts[parts.len().saturating_sub(2)..];
        let joined = tail.join("/");
        if joined.chars().count() <= BUDGET {
            return joined;
        }
        let last = tail.last().copied().unwrap_or_default();
        let n = last.chars().count();
        if n <= BUDGET {
            return last.to_string();
        }
        let keep: String = last.chars().skip(n - (BUDGET - 1)).collect();
        format!("…{keep}")
    }

    pub fn dir(&self) -> &str {
        self.path
            .rsplit_once('/')
            .map(|(dir, _)| dir)
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub branch: String,
    pub state: DirtState,
    pub files: Vec<GitFile>,
}
