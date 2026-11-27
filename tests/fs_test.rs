// End-to-end tests for every callback in src/rules/default_callbacks.rs.
//
// Callbacks can't be called directly from a test: Context needs a
// &fuser::Request, which has no public constructor. Instead each test mounts
// vffs with a rule that uses the callback and checks what a normal file
// operation does.
//
// Needs FUSE (/dev/fuse + fusermount). Run with: cargo test --test callbacks_test
//
// Most tests pair the condition under test with FailWithCode(PROBE) and check
// whether the operation failed with PROBE. EXDEV is used because none of these
// operations would ever return it naturally.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use fuser::BackgroundSession;
use indexmap::IndexMap;
use tempfile::{tempdir, TempDir};
use vffs::{
    fs::fault_fs::FaultInjectionFileSystem,
    rules::{
        gen_fn::{RngSeed, TimeAmount},
        rules::{
            ActionType, MatchType, PreparseCondition as C, PreparseModification as M, PreparseRule,
            PreparseRules, RuleType,
        },
    },
};

const PROBE: i32 = libc::EXDEV;


// ===========================================================================
// Harness
// ===========================================================================

struct TempFs {
    // Declared first so it unmounts before the temp dirs are deleted
    _session: BackgroundSession,
    src: TempDir,
    mnt: TempDir,
    _rules: TempDir,
}

impl TempFs {
    // Mount with every default callback registered explicitly.
    // (with_default_actions() is currently empty, so it can't be relied on;
    // see default_actions_are_registered below.)
    fn new(rules: Vec<(&str, PreparseRule)>) -> Self {
        Self::mount(rules, register_all)
    }

    fn mount(
        rules: Vec<(&str, PreparseRule)>,
        register: impl FnOnce(FaultInjectionFileSystem) -> FaultInjectionFileSystem,
    ) -> Self {
        let src = tempdir().unwrap();
        let mnt = tempdir().unwrap();
        let rules_dir = tempdir().unwrap();
        let rules_file = rules_dir.path().join("rules.yaml");
        // Always write the file: RuleManager::new calls process::exit(0) if
        // it's missing, which would kill the whole test binary
        write_rules(&rules_file, rules);

        let vfs = FaultInjectionFileSystem::new(src.path(), mnt.path(), &rules_file, Duration::from_secs(1))
            .unwrap();
        let session = register(vfs).spawn_mount().unwrap();

        fs::write(src.path().join("a"), b"hello world").unwrap();

        Self { _session: session, src, mnt, _rules: rules_dir }
    }

    fn src(&self, p: impl AsRef<Path>) -> PathBuf {
        self.src.path().join(p)
    }

    fn mnt(&self, p: impl AsRef<Path>) -> PathBuf {
        self.mnt.path().join(p)
    }
}

fn register_all(vfs: FaultInjectionFileSystem) -> FaultInjectionFileSystem {
    vfs.with_default_conditions()
        .with_default_actions()
}

fn write_rules(path: &Path, rules: Vec<(&str, PreparseRule)>) {
    let map: IndexMap<String, PreparseRule> = rules
        .into_iter()
        .map(|(name, r)| (name.to_string(), r))
        .collect();
    fs::write(path, yaml_serde::to_string(&PreparseRules(map)).unwrap()).unwrap();
}

fn rule(rule_type: RuleType, conditions: Vec<C>, modifications: Vec<M>) -> PreparseRule {
    PreparseRule {
        inactive: false,
        rule_type,
        condition_match_type: MatchType::MatchAll,
        conditions,
        modifications,
    }
}

fn errno<T>(res: io::Result<T>) -> Option<i32> {
    res.err().and_then(|e| e.raw_os_error())
}

// Mounts with one post-action rule: [ActionType(Open), cond] => FailWithCode(PROBE),
// opens a file and reports whether the rule fired
fn fires_on_open(cond: C) -> bool {
    let tfs = TempFs::new(vec![(
        "probe",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::Open), cond], vec![M::FailWithCode(PROBE)]),
    )]);

    match errno(fs::File::open(tfs.mnt("a"))) {
        Some(PROBE) => true,
        None => false,
        Some(other) => panic!("open failed with unexpected errno {other}"),
    }
}

// Thread id of the caller. FUSE reports the calling *thread's* id as the
// request pid, and test threads aren't the main thread.
fn tid() -> u32 {
    unsafe { libc::gettid() as u32 }
}


// ===========================================================================
// match_action
// ===========================================================================

#[test]
fn match_action_fires_only_on_matching_action() {
    let tfs = TempFs::new(vec![(
        "fail open",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::Open)], vec![M::FailWithCode(PROBE)]),
    )]);

    assert_eq!(errno(fs::File::open(tfs.mnt("a"))), Some(PROBE));
    // Lookup/getattr/readdir are untouched
    assert!(fs::metadata(tfs.mnt("a")).is_ok());
    assert!(fs::read_dir(tfs.mnt.path()).is_ok());
}

#[test]
fn match_action_all_matches_everything() {
    let tfs = TempFs::new(vec![(
        "fail all",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::All)], vec![M::FailWithCode(PROBE)]),
    )]);

    assert_eq!(errno(fs::metadata(tfs.mnt("a"))), Some(PROBE));
}

#[test]
fn match_action_none_matches_nothing() {
    assert!(!fires_on_open(C::ActionType(ActionType::None)));
}

#[test]
fn match_action_works_in_pre_action_rules() {
    // match_action is registered for pre-actions too; prove it by delaying
    let tfs = TempFs::new(vec![(
        "slow open",
        rule(
            RuleType::PreAction,
            vec![C::ActionType(ActionType::Open)],
            vec![M::DelayBy(TimeAmount::Millis(150))],
        ),
    )]);

    let start = Instant::now();
    fs::File::open(tfs.mnt("a")).unwrap();
    assert!(start.elapsed() >= Duration::from_millis(150));
}


// ===========================================================================
// probability
// ===========================================================================
//
// The RNG is a static shared by every rule and every test in this binary, so
// only the 0% and 100% cases are deterministic. Seeds can't be tested until
// RNG state is per rule.

#[test]
fn probability_100_always_fires() {
    let tfs = TempFs::new(vec![(
        "always",
        rule(
            RuleType::PostAction,
            vec![C::ActionType(ActionType::Open), C::Probability(100.0, RngSeed::Seeded(1))],
            vec![M::FailWithCode(PROBE)],
        ),
    )]);

    for _ in 0..20 {
        assert_eq!(errno(fs::File::open(tfs.mnt("a"))), Some(PROBE));
    }
}

#[test]
fn probability_0_never_fires() {
    let tfs = TempFs::new(vec![(
        "never",
        rule(
            RuleType::PostAction,
            vec![C::ActionType(ActionType::Open), C::Probability(0.0, RngSeed::Seeded(1))],
            vec![M::FailWithCode(PROBE)],
        ),
    )]);

    for _ in 0..20 {
        fs::File::open(tfs.mnt("a")).unwrap();
    }
}

#[test]
fn probability_50_fires_sometimes() {
    let tfs = TempFs::new(vec![(
        "half",
        rule(
            RuleType::PostAction,
            vec![C::ActionType(ActionType::Open), C::Probability(50.0, RngSeed::CurrentTimeAsSeed)],
            vec![M::FailWithCode(PROBE)],
        ),
    )]);

    let trials = 200;
    let fired = (0..trials)
        .filter(|_| errno(fs::File::open(tfs.mnt("a"))) == Some(PROBE))
        .count();

    // Wide bounds: chance of a false failure is far below 1 in a million
    assert!((40..=160).contains(&fired), "fired {fired}/{trials}");
}

// Run this one on its own until the fix lands: the panic happens while the
// shared RNG mutex is held, which poisons it and breaks every other
// probability test in the same run
#[test]
fn probability_out_of_range_does_not_break_the_mount() {
    let tfs = TempFs::new(vec![(
        "bad pct",
        rule(
            RuleType::PostAction,
            vec![C::ActionType(ActionType::Open), C::Probability(150.0, RngSeed::Seeded(1))],
            vec![M::FailWithCode(PROBE)],
        ),
    )]);

    // An invalid argument should make the condition error (rule doesn't fire),
    // not crash the request thread
    fs::File::open(tfs.mnt("a")).unwrap();
    assert_eq!(fs::read(tfs.mnt("a")).unwrap(), b"hello world");
}


// ===========================================================================
// match_pid / match_uid / match_gid
// ===========================================================================

#[test]
fn match_pid_matches_calling_thread() {
    assert!(fires_on_open(C::MatchPid(tid())));
}

#[test]
fn match_pid_ignores_other_pids() {
    assert!(!fires_on_open(C::MatchPid(u32::MAX)));
}

#[test]
fn match_uid_matches_caller() {
    assert!(fires_on_open(C::MatchUid(unsafe { libc::geteuid() })));
}

#[test]
fn match_uid_ignores_other_uids() {
    assert!(!fires_on_open(C::MatchUid(u32::MAX - 1)));
}

#[test]
fn match_gid_matches_caller() {
    assert!(fires_on_open(C::MatchGid(unsafe { libc::getegid() })));
}

#[test]
fn match_gid_ignores_other_gids() {
    assert!(!fires_on_open(C::MatchGid(u32::MAX - 1)));
}


// ===========================================================================
// is_err / err_matches
// ===========================================================================

#[test]
fn is_err_fires_only_on_failed_operations() {
    let tfs = TempFs::new(vec![(
        "rewrite errors",
        rule(RuleType::PostAction, vec![C::IsErr], vec![M::FailWithCode(PROBE)]),
    )]);

    // Missing file: lookup fails with ENOENT, rule rewrites it
    assert_eq!(errno(fs::metadata(tfs.mnt("missing"))), Some(PROBE));
    // Successful operations are untouched
    assert_eq!(fs::read(tfs.mnt("a")).unwrap(), b"hello world");
}

#[test]
fn err_matches_fires_on_matching_errno() {
    let tfs = TempFs::new(vec![(
        "enoent",
        rule(RuleType::PostAction, vec![C::ErrMatches(libc::ENOENT)], vec![M::FailWithCode(PROBE)]),
    )]);

    assert_eq!(errno(fs::metadata(tfs.mnt("missing"))), Some(PROBE));
    assert!(fs::metadata(tfs.mnt("a")).is_ok());
}

#[test]
fn err_matches_ignores_other_errnos() {
    let tfs = TempFs::new(vec![(
        "eacces",
        rule(RuleType::PostAction, vec![C::ErrMatches(libc::EACCES)], vec![M::FailWithCode(PROBE)]),
    )]);

    assert_eq!(errno(fs::metadata(tfs.mnt("missing"))), Some(libc::ENOENT));
}


// ===========================================================================
// duration_over
// ===========================================================================

#[test]
fn duration_over_ignores_fast_operations() {
    assert!(!fires_on_open(C::DurationOver(TimeAmount::Secs(3600))));
}

#[test]
fn duration_over_fires_on_slow_operations() {
    // The pre-action delay counts towards action_duration, because
    // pre_action_time is taken before pre-actions run
    let tfs = TempFs::new(vec![
        (
            "slow open",
            rule(
                RuleType::PreAction,
                vec![C::ActionType(ActionType::Open)],
                vec![M::DelayBy(TimeAmount::Millis(100))],
            ),
        ),
        (
            "fail if slow",
            rule(
                RuleType::PostAction,
                vec![C::ActionType(ActionType::Open), C::DurationOver(TimeAmount::Millis(50))],
                vec![M::FailWithCode(PROBE)],
            ),
        ),
    ]);

    assert_eq!(errno(fs::File::open(tfs.mnt("a"))), Some(PROBE));
}


// ===========================================================================
// delay_by / delay_by_pct
// ===========================================================================
//
// Only lower bounds are checked; upper bounds would be flaky on busy CI.

#[test]
fn delay_by_as_pre_action() {
    let tfs = TempFs::new(vec![(
        "slow open",
        rule(RuleType::PreAction, vec![C::ActionType(ActionType::Open)], vec![M::DelayBy(TimeAmount::Millis(200))]),
    )]);

    let start = Instant::now();
    fs::File::open(tfs.mnt("a")).unwrap();
    assert!(start.elapsed() >= Duration::from_millis(200));
}

#[test]
fn delay_by_as_post_action() {
    let tfs = TempFs::new(vec![(
        "slow open",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::Open)], vec![M::DelayBy(TimeAmount::Millis(200))]),
    )]);

    let start = Instant::now();
    fs::File::open(tfs.mnt("a")).unwrap();
    assert!(start.elapsed() >= Duration::from_millis(200));
}

#[test]
fn delay_by_only_delays_matching_actions() {
    let tfs = TempFs::new(vec![(
        "slow open",
        rule(RuleType::PreAction, vec![C::ActionType(ActionType::Open)], vec![M::DelayBy(TimeAmount::Secs(5))]),
    )]);

    let start = Instant::now();
    fs::metadata(tfs.mnt("a")).unwrap();
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn delay_by_pct_scales_with_action_duration() {
    // A 100ms pre-action delay makes the open take >= 100ms; 100% then adds
    // another >= 100ms, for >= 200ms total
    let tfs = TempFs::new(vec![
        (
            "base delay",
            rule(RuleType::PreAction, vec![C::ActionType(ActionType::Open)], vec![M::DelayBy(TimeAmount::Millis(100))]),
        ),
        (
            "double it",
            rule(RuleType::PostAction, vec![C::ActionType(ActionType::Open)], vec![M::DelayByPct(100.0)]),
        ),
    ]);

    let start = Instant::now();
    fs::File::open(tfs.mnt("a")).unwrap();
    assert!(start.elapsed() >= Duration::from_millis(200));
}


// ===========================================================================
// map_errno
// ===========================================================================

#[test]
fn map_errno_rewrites_matching_errno() {
    let tfs = TempFs::new(vec![(
        "enoent to probe",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::All)], vec![M::MapErrno(libc::ENOENT, PROBE)]),
    )]);

    assert_eq!(errno(fs::metadata(tfs.mnt("missing"))), Some(PROBE));
}

#[test]
fn map_errno_leaves_other_errnos_alone() {
    let tfs = TempFs::new(vec![(
        "eacces to probe",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::All)], vec![M::MapErrno(libc::EACCES, PROBE)]),
    )]);

    assert_eq!(errno(fs::metadata(tfs.mnt("missing"))), Some(libc::ENOENT));
}

#[test]
fn map_errno_leaves_successes_alone() {
    let tfs = TempFs::new(vec![(
        "enoent to probe",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::All)], vec![M::MapErrno(libc::ENOENT, PROBE)]),
    )]);

    assert_eq!(fs::read(tfs.mnt("a")).unwrap(), b"hello world");
}


// ===========================================================================
// fail_with_code
// ===========================================================================

#[test]
fn fail_with_code_returns_the_errno() {
    assert!(fires_on_open(C::ActionType(ActionType::Open)));
}

// fail_with_code is post-action only: the real operation has already run.
// This documents that, so it shows up if you add a pre-action short-circuit.
#[test]
fn fail_with_code_runs_after_the_real_operation() {
    use std::os::unix::fs::PermissionsExt;

    let tfs = TempFs::new(vec![(
        "fail setattr",
        rule(RuleType::PostAction, vec![C::ActionType(ActionType::Setattr)], vec![M::FailWithCode(PROBE)]),
    )]);
    fs::set_permissions(tfs.src("a"), fs::Permissions::from_mode(0o644)).unwrap();

    let res = fs::set_permissions(tfs.mnt("a"), fs::Permissions::from_mode(0o600));
    assert_eq!(errno(res), Some(PROBE));

    let mode = fs::metadata(tfs.src("a")).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "chmod reported failure but should still have happened");
}


// ===========================================================================
// corrupt_read
// ===========================================================================

fn corrupt_mount(pct: f64) -> TempFs {
    let tfs = TempFs::new(vec![(
        "corrupt",
        rule(
            RuleType::PostAction,
            vec![C::ActionType(ActionType::Read)],
            vec![M::CorruptRead(pct, RngSeed::Seeded(1))],
        ),
    )]);
    fs::write(tfs.src("data"), vec![0x5A; 4096]).unwrap();
    tfs
}

// Fails on the current code, which always clears bit 0 of every byte
#[test]
fn corrupt_read_0_pct_leaves_data_untouched() {
    let tfs = corrupt_mount(0.0);
    assert_eq!(fs::read(tfs.mnt("data")).unwrap(), vec![0x5A; 4096]);
}

// Fails on the current code, which can only clear bits, never set them
#[test]
fn corrupt_read_100_pct_flips_every_bit() {
    let tfs = corrupt_mount(100.0);
    assert_eq!(fs::read(tfs.mnt("data")).unwrap(), vec![!0x5Au8; 4096]);
}

#[test]
fn corrupt_read_partial_changes_some_bytes() {
    let tfs = corrupt_mount(10.0);
    let data = fs::read(tfs.mnt("data")).unwrap();
    assert_eq!(data.len(), 4096, "corruption must not change the length");
    assert_ne!(data, vec![0x5A; 4096]);
}

#[test]
fn corrupt_read_does_not_touch_the_source() {
    let tfs = corrupt_mount(100.0);
    fs::read(tfs.mnt("data")).unwrap();
    assert_eq!(fs::read(tfs.src("data")).unwrap(), vec![0x5A; 4096]);
}


// ===========================================================================
// log_* (smoke tests)
// ===========================================================================
//
// The output comes from the FUSE thread, which the test harness doesn't
// capture, so these only check that logging doesn't break any operation.

#[test]
fn log_callbacks_do_not_affect_operations() {
    let tfs = TempFs::new(vec![
        (
            "log pre",
            rule(RuleType::PreAction, vec![C::ActionType(ActionType::All)], vec![M::LogActionType]),
        ),
        (
            "log post",
            rule(
                RuleType::PostAction,
                vec![C::ActionType(ActionType::All)],
                vec![
                    M::LogActionType,
                    M::LogActionCtx(false),
                    M::LogActionCtx(true),
                    M::LogReply(false),
                    M::LogReply(true),
                    M::LogCtx(false),
                    M::LogCtx(true),
                ],
            ),
        ),
    ]);

    assert_eq!(fs::read(tfs.mnt("a")).unwrap(), b"hello world");
    assert!(fs::metadata(tfs.mnt("a")).is_ok());
    assert_eq!(errno(fs::metadata(tfs.mnt("missing"))), Some(libc::ENOENT));
    assert_eq!(fs::read_dir(tfs.mnt.path()).unwrap().count(), 1);
}


// ===========================================================================
// Registration
// ===========================================================================


#[test]
fn default_conditions_are_registered() {
    let tfs_rules = vec![(
        "every condition",
        rule(
            RuleType::PostAction,
            vec![
                C::ActionType(ActionType::None),
                C::Probability(1.0, RngSeed::Seeded(1)),
                C::MatchPid(1),
                C::MatchUid(1),
                C::MatchGid(1),
                C::IsErr,
                C::ErrMatches(libc::EIO),
                C::DurationOver(TimeAmount::Secs(1)),
            ],
            vec![],
        ),
    )];

    let _tfs = TempFs::mount(tfs_rules, |vfs| vfs.with_default_conditions());
}
