/// Marker file that identifies a directory as a taskmd tasks directory.
pub const TEMPLATE_FILENAME: &str = "_TEMPLATE.md";

/// Conventional tasks-directory name — the default, and the tie-breaker when
/// `discover` finds more than one candidate.
pub const DEFAULT_TASKS_DIR_NAME: &str = "tasks";

/// Valid task statuses — alphabetical order matches Python's `sorted(VALID_STATUSES)`.
pub const VALID_STATUSES: &[&str] = &[
    "blocked",
    "brainstorming",
    "done",
    "in-progress",
    "ready",
    "wont-do",
];

/// Valid task priorities, ordered p0 (highest) to p4 (lowest).
pub const VALID_PRIORITIES: &[&str] = &["p0", "p1", "p2", "p3", "p4"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
    P4,
}

impl Priority {
    pub const ALL: &'static [Priority] = &[
        Priority::P0,
        Priority::P1,
        Priority::P2,
        Priority::P3,
        Priority::P4,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::P0 => "p0",
            Priority::P1 => "p1",
            Priority::P2 => "p2",
            Priority::P3 => "p3",
            Priority::P4 => "p4",
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Priority {
    type Err = crate::error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "p0" => Ok(Priority::P0),
            "p1" => Ok(Priority::P1),
            "p2" => Ok(Priority::P2),
            "p3" => Ok(Priority::P3),
            "p4" => Ok(Priority::P4),
            other => Err(crate::error::Error::InvalidPriority {
                got: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Status {
    Blocked,
    Brainstorming,
    Done,
    InProgress,
    Ready,
    WontDo,
}

impl Status {
    pub const ALL: &'static [Status] = &[
        Status::Blocked,
        Status::Brainstorming,
        Status::Done,
        Status::InProgress,
        Status::Ready,
        Status::WontDo,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Blocked => "blocked",
            Status::Brainstorming => "brainstorming",
            Status::Done => "done",
            Status::InProgress => "in-progress",
            Status::Ready => "ready",
            Status::WontDo => "wont-do",
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Status {
    type Err = crate::error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "blocked" => Ok(Status::Blocked),
            "brainstorming" => Ok(Status::Brainstorming),
            "done" => Ok(Status::Done),
            "in-progress" => Ok(Status::InProgress),
            "ready" => Ok(Status::Ready),
            "wont-do" => Ok(Status::WontDo),
            other => Err(crate::error::Error::InvalidStatus {
                got: other.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn priority_str_roundtrip() {
        for s in VALID_PRIORITIES {
            let parsed = Priority::from_str(s).expect("valid priority should parse");
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn status_str_roundtrip() {
        for s in VALID_STATUSES {
            let parsed = Status::from_str(s).expect("valid status should parse");
            assert_eq!(parsed.as_str(), *s);
        }
    }

    #[test]
    fn priority_all_aligns_with_const() {
        let from_enum: Vec<&'static str> = Priority::ALL.iter().map(|p| p.as_str()).collect();
        let from_const: Vec<&'static str> = VALID_PRIORITIES.to_vec();
        assert_eq!(from_enum, from_const);
    }

    #[test]
    fn status_all_aligns_with_const() {
        let from_enum: Vec<&'static str> = Status::ALL.iter().map(|s| s.as_str()).collect();
        let from_const: Vec<&'static str> = VALID_STATUSES.to_vec();
        assert_eq!(from_enum, from_const);
    }

    #[test]
    fn priority_invalid_string_errs() {
        assert!(Priority::from_str("p5").is_err());
    }

    #[test]
    fn status_invalid_string_errs() {
        assert!(Status::from_str("pending").is_err());
    }
}
