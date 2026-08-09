# Replace Git CLI history traversal with gix

Spike the latest pure-Rust gix/gitoxide library as the implementation of locally known Git history traversal for task ID allocation. Production allocation must not spawn the Git executable.

Build a differential end-to-end acceptance harness around concrete repository fixtures. Each fixture must assert the same observable next-ID outcome from the Git CLI reference backend and the gix backend rather than testing implementation details.

## Acceptance fixtures

- [x] Sibling local branches from one base
- [x] Remote-tracking branch not checked out locally
- [x] Commit reachable only through a reflog
- [x] Task renamed and deleted in history remains reserved
- [x] Shallow repository sees only locally available history
- [x] Non-Git task directory falls back to visible files
- [x] Foreign-prefix and malformed filenames do not affect the local sequence

## Done when

- [x] Production Rust code contains no Git CLI process invocation for allocation
- [x] Differential harness passes for every shared fixture
- [x] Dependency, MSRV, build-size, and traversal-time impact are measured
- [x] Rust and Python suites pass

## Spike measurements

Measured on macOS arm64 against commit `2788607`, using `gix 0.86.0` with
default features disabled and only `revision` and `sha1` enabled:

| Measure | Git CLI baseline | gix spike | Impact |
|---|---:|---:|---:|
| `taskmd-core` dependency-tree packages | 46 | 170 | +124 |
| Optimized `taskmd-core` rlib | 524,920 B | 695,576 B | +32% |
| CPython 3.12 wheel | 977,804 B | 1,697,790 B | +74% |
| Median history scan, 25 runs | 17.54 ms | 19.44 ms | +11% |
| MSRV | Rust 1.75 | Rust 1.85 | +10 releases |

The CLI implementation remains test-only as the differential oracle. Production
allocation uses gix exclusively.
