use super::*;
fn fixture() -> (tempfile::TempDir, Layout) {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().canonicalize().unwrap().join(".dev");
    fs::create_dir(&directory).unwrap();
    let source = temp.path().join("server.rs");
    fs::write(&source,r#"use std::{net::TcpListener,io::{Read,Write}};
fn main(){let config=std::env::args().nth(2).unwrap();let port=std::fs::read_to_string(config).unwrap();
let listener=TcpListener::bind(format!("127.0.0.1:{}",port.trim())).unwrap();
for mut stream in listener.incoming().flatten(){let _=stream.read(&mut [0;1024]);let _=stream.write_all(b"HTTP/1.1 200 OK
Content-Length: 2
Connection: close

OK");}}"#).unwrap();
    let executable = directory.join("cli-proxy-api");
    assert!(Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .status()
        .unwrap()
        .success());
    let port = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let config = directory.join("config.yaml");
    fs::write(&config, port.to_string()).unwrap();
    let layout = Layout {
        repository: temp.path().into(),
        directory,
        executable,
        config,
        port,
    };
    (temp, layout)
}
#[test]
fn bad_build_preserves_live_service_and_failed_upgrade_restores_old_binary_and_config() {
    let (_temp, layout) = fixture();
    launch(&layout).unwrap();
    let pid = owner(&layout).unwrap().unwrap();
    let failed = layout.directory.join("fake-go");
    fs::write(
        &failed,
        "#!/bin/sh
exit 1
",
    )
    .unwrap();
    fs::set_permissions(&failed, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(build_with(&layout, &failed).is_err());
    assert_eq!(owner(&layout).unwrap(), Some(pid));
    let mut staged = tempfile::NamedTempFile::new_in(&layout.directory).unwrap();
    use std::io::Write;
    staged
        .write_all(
            b"#!/bin/sh
printf changed > \"$2\"
exit 1
",
        )
        .unwrap();
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let error = replace_and_restart(&layout, staged).unwrap_err();
    assert!(error.contains("已恢复原程序"), "{error}");
    assert_eq!(
        fs::read_to_string(&layout.config).unwrap(),
        layout.port.to_string()
    );
    assert!(owner(&layout).unwrap().is_some());
    stop(&layout).unwrap();
}
#[test]
fn occupied_port_is_never_terminated_or_replaced() {
    let (_temp, layout) = fixture();
    let foreign = TcpListener::bind(("127.0.0.1", layout.port)).unwrap();
    assert!(restart(&layout).is_err());
    assert_eq!(foreign.local_addr().unwrap().port(), layout.port);
}

#[test]
fn supervised_service_is_taken_over_without_restarting_its_sibling() {
    let (_temp, layout) = fixture();
    let supervisor = layout.directory.join("dev-test.py");
    fs::write(&supervisor, r#"import fcntl,subprocess,signal,sys,time
from pathlib import Path
root=Path(sys.argv[1])
lock=open(root/'dev.lock','w');fcntl.flock(lock,fcntl.LOCK_EX)
cpa=subprocess.Popen([str(root/'cli-proxy-api'),'--config',str(root/'config.yaml')],start_new_session=True)
cpm=subprocess.Popen(['/bin/sleep','60'],start_new_session=True)
(root/'test-cpm.pid').write_text(str(cpm.pid))
def stop(*args):
    for child in [cpa,cpm]: child.terminate()
    for child in [cpa,cpm]: child.wait()
    sys.exit(0)
signal.signal(signal.SIGTERM,stop)
while True: time.sleep(.1)
"#).unwrap();
    let mut launcher = Command::new("/usr/bin/python3")
        .arg(supervisor)
        .arg(&layout.directory)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while owner(&layout).ok().flatten().is_none() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    assert!(owner(&layout).unwrap().is_some());
    let sibling: i32 = fs::read_to_string(layout.directory.join("test-cpm.pid"))
        .unwrap()
        .parse()
        .unwrap();
    restart(&layout).unwrap();
    assert!(launcher.wait().unwrap().success());
    assert_eq!(unsafe { libc::kill(sibling, 0) }, -1);
    assert!(owner(&layout).unwrap().is_some());
    stop(&layout).unwrap();
}
