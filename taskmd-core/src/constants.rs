/// Valid task statuses — alphabetical order matches Python's `sorted(VALID_STATUSES)`.
pub const VALID_STATUSES: &[&str] =
    &["blocked", "brainstorming", "done", "in-progress", "ready", "wont-do"];

/// Valid task priorities, ordered p0 (highest) to p4 (lowest).
pub const VALID_PRIORITIES: &[&str] = &["p0", "p1", "p2", "p3", "p4"];

/// Valid frontmatter field names — alphabetical order.
pub const VALID_FIELDS: &[&str] = &["artifact", "created", "priority", "status"];
