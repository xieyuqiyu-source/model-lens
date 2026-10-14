use super::*;
use crate::management::cpa::tests::{command, commit};
#[test]
fn comparison_fetches_tracking_branch_without_merging_and_handles_failures() {
    let temp = tempfile::tempdir().unwrap();
    let remote = temp.path().join("remote");
    std::fs::create_dir(&remote).unwrap();
    command(&remote, &["init", "-b", "main"]);
    commit(&remote, "first");
    let work = temp.path().join("work");
    command(
        temp.path(),
        &["clone", remote.to_str().unwrap(), work.to_str().unwrap()],
    );
    let first = local_git(&work, &["rev-parse", "HEAD"]).unwrap();
    let build = (Some(first.clone()), Some(false));
    assert_eq!(inspect(&work, build.clone()).unwrap().status, "current");
    command(&remote, &["checkout", "-b", "keep"]);
    command(&remote, &["branch", "-D", "main"]);
    let deleted = inspect(&work, build.clone()).unwrap();
    assert_eq!(deleted.status, "unknown");
    assert!(deleted.remote_error.is_some());
    assert_eq!(deleted.checked_at, 0);
    command(&remote, &["checkout", "-b", "main"]);
    command(&remote, &["branch", "-D", "keep"]);
    commit(&remote, "second");
    let old = inspect(&work, build.clone()).unwrap();
    assert_eq!(old.status, "behind");
    assert_eq!(old.behind, 1);
    assert_eq!(local_git(&work, &["rev-parse", "HEAD"]).unwrap(), first);
    assert!(!work.join("second").exists());
    command(&work, &["merge", "--ff-only", "origin/main"]);
    assert_eq!(inspect(&work, build.clone()).unwrap().status, "unbuilt");
    let latest = local_git(&work, &["rev-parse", "HEAD"]).unwrap();
    assert_eq!(
        inspect(&work, (Some(latest.clone()), Some(false)))
            .unwrap()
            .status,
        "current"
    );
    std::fs::write(work.join("local-change"), "keep").unwrap();
    assert_eq!(
        inspect(&work, (Some(latest), Some(false))).unwrap().status,
        "modified"
    );
    assert_eq!(
        std::fs::read_to_string(work.join("local-change")).unwrap(),
        "keep"
    );
    commit(&work, "ahead");
    assert_eq!(inspect(&work, build.clone()).unwrap().status, "ahead");
    commit(&remote, "third");
    assert_eq!(inspect(&work, build.clone()).unwrap().status, "diverged");
    command(
        &work,
        &[
            "remote",
            "set-url",
            "origin",
            temp.path().join("missing").to_str().unwrap(),
        ],
    );
    let unavailable = inspect(&work, build.clone()).unwrap();
    assert_eq!(unavailable.status, "unknown");
    assert!(unavailable.remote.is_some());
    assert!(unavailable.remote_error.is_some());
    assert_eq!(unavailable.checked_at, 0);
    command(&work, &["checkout", "--detach"]);
    assert_eq!(inspect(&work, build).unwrap().status, "unknown");
}
#[test]
fn build_metadata_never_projects_keys_and_unknown_or_dirty_is_not_current() {
    let sha = "a".repeat(40);
    let output = format!("path /synthetic/cpa\n\tbuild\tvcs.revision={sha}\n\tbuild\tvcs.modified=false\n\tbuild\t-secret=not-for-ui");
    assert_eq!(build_info(&output), (Some(sha.clone()), Some(false)));
    assert_eq!(build_info("build\tvcs.revision=malformed"), (None, None));
    let v = Versions {
        source: sha.clone(),
        local: Some(sha.clone()),
        remote: Some(sha),
        branch: "main".into(),
        upstream: Some("origin/main".into()),
        ahead: 0,
        behind: 0,
        dirty: false,
        status: "unknown",
        remote_error: None,
        checked_at: 1,
    };
    assert_eq!(classify(&v, Some(true)), "modified");
    assert_eq!(classify(&v, None), "unknown");
}
