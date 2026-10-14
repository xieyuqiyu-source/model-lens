use super::*;
fn command(path: &Path, args: &[&str]) {
    assert!(Command::new("/usr/bin/git")
        .current_dir(path)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Synthetic")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Synthetic")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
}
fn commit(path: &Path, name: &str) {
    fs::write(path.join(name), name).unwrap();
    command(path, &["add", "."]);
    command(path, &["commit", "-m", name]);
}
#[test]
fn pull_fast_forwards_only_and_preserves_dirty_diverged_and_detached_repositories() {
    let temp = tempfile::tempdir().unwrap();
    let upstream = temp.path().join("upstream");
    fs::create_dir(&upstream).unwrap();
    command(&upstream, &["init", "-b", "main"]);
    fs::write(
        upstream.join("go.mod"),
        "module github.com/router-for-me/CLIProxyAPI/v8\n",
    )
    .unwrap();
    commit(&upstream, "initial");
    let work = temp.path().join("CLIProxyApi");
    command(
        temp.path(),
        &["clone", upstream.to_str().unwrap(), work.to_str().unwrap()],
    );
    assert!(!pull(&work).unwrap().updated);
    commit(&upstream, "new");
    let updated = pull(&work).unwrap();
    assert!(updated.updated);
    assert_ne!(updated.before, updated.after);
    let head = updated.after;
    fs::write(work.join("local"), "keep").unwrap();
    assert!(pull(&work).err().unwrap().contains("未提交"));
    assert_eq!(fs::read_to_string(work.join("local")).unwrap(), "keep");
    fs::remove_file(work.join("local")).unwrap();
    command(&work, &["checkout", "--detach"]);
    assert!(pull(&work).err().unwrap().contains("分支"));
    command(&work, &["checkout", "main"]);
    commit(&work, "local-commit");
    commit(&upstream, "remote-commit");
    let before = local_git(&work, &["rev-parse", "HEAD"]).unwrap();
    assert!(pull(&work).is_err());
    assert_eq!(local_git(&work, &["rev-parse", "HEAD"]).unwrap(), before);
    assert_ne!(head, before);
    assert!(work.join("local-commit").exists());
    assert!(!work.join("remote-commit").exists());
    // Local hooks must not run as a side effect of updating source.
    assert!(local_git(&work, &["status", "--porcelain"])
        .unwrap()
        .is_empty());
}
#[test]
fn source_detection_and_management_url_are_fixed_to_the_connected_service() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let repo = root.join("CLIProxyApi");
    fs::create_dir(&repo).unwrap();
    command(&repo, &["init", "-b", "main"]);
    fs::write(
        repo.join("go.mod"),
        "module github.com/router-for-me/CLIProxyAPI/v8\n",
    )
    .unwrap();
    assert_eq!(
        repository(&root.join(".dev/cpa")).unwrap(),
        repo.canonicalize().unwrap()
    );
    assert!(repository(&root.join("unrelated/cpa")).is_err());
    fs::write(repo.join("go.mod"), "module unrelated\n").unwrap();
    assert!(repository(&root.join(".dev/cpa")).is_err());
    assert_eq!(
        management_url(8317),
        "http://127.0.0.1:8317/management.html"
    );
}

#[test]
fn pull_never_overwrites_ignored_configuration_or_runs_checkout_hooks() {
    let temp = tempfile::tempdir().unwrap();
    let upstream = temp.path().join("upstream");
    fs::create_dir(&upstream).unwrap();
    command(&upstream, &["init", "-b", "main"]);
    fs::write(
        upstream.join("go.mod"),
        "module github.com/router-for-me/CLIProxyAPI/v8\n",
    )
    .unwrap();
    fs::write(upstream.join(".gitignore"), "config.yaml\n").unwrap();
    commit(&upstream, "initial");
    let work = temp.path().join("CLIProxyApi");
    command(
        temp.path(),
        &["clone", upstream.to_str().unwrap(), work.to_str().unwrap()],
    );
    fs::write(work.join("config.yaml"), "synthetic-local-config").unwrap();
    fs::write(upstream.join("config.yaml"), "synthetic-remote-config").unwrap();
    command(&upstream, &["add", "-f", "config.yaml"]);
    command(&upstream, &["commit", "-m", "remote-config"]);
    let before = local_git(&work, &["rev-parse", "HEAD"]).unwrap();
    assert!(pull(&work).is_err());
    assert_eq!(local_git(&work, &["rev-parse", "HEAD"]).unwrap(), before);
    assert_eq!(
        fs::read_to_string(work.join("config.yaml")).unwrap(),
        "synthetic-local-config"
    );
    fs::remove_file(work.join("config.yaml")).unwrap();
    let hook = work.join(".git/hooks/post-merge");
    fs::write(&hook, "#!/bin/sh\ntouch hook-ran\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(hook, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(pull(&work).unwrap().updated);
    assert!(!work.join("hook-ran").exists());
}
