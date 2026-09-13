use std::process::Command;
use tempfile::TempDir;

#[test]
fn redirected_diagnostics_use_stderr_without_color() {
    let dir = TempDir::new().unwrap();
    for no_color in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kibob"));
        command
            .env_clear()
            .current_dir(dir.path())
            .env_remove("FORCE_COLOR")
            .env_remove("CLICOLOR_FORCE")
            .env_remove("NO_COLOR")
            .args(["--env", "missing.env", "import", "tools", "missing.json"]);
        if no_color {
            command.env("NO_COLOR", "1").env("FORCE_COLOR", "1");
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(
            output.stdout.is_empty(),
            "diagnostics must not enter stdout"
        );
        assert!(!output.stderr.is_empty());
        assert!(
            !output.stderr.contains(&0x1b),
            "captured diagnostics contain ANSI"
        );
    }
}

#[test]
fn invalid_deadlines_fail_before_network_access() {
    let dir = TempDir::new().unwrap();
    let manifest = "{\"objects\": []}";
    std::fs::write(dir.path().join("manifest.json"), manifest).unwrap();
    for name in ["KIBANA_REQUEST_TIMEOUT", "KIBANA_CONNECT_TIMEOUT"] {
        for value in ["0", "-1", "1.5", "invalid"] {
            for action in ["auth", "migrate"] {
                let output = Command::new(env!("CARGO_BIN_EXE_kibob"))
                    .env_clear()
                    .env("KIBANA_URL", "http://127.0.0.1:1")
                    .env(name, value)
                    .current_dir(dir.path())
                    .args(["--env", "missing.env", action])
                    .output()
                    .unwrap();
                assert_eq!(output.status.code(), Some(1));
                assert!(output.stdout.is_empty());
                let error = String::from_utf8_lossy(&output.stderr);
                assert!(error.contains(&format!("{name} must be a positive integer")));
                assert!(!dir.path().join("default").exists());
                assert_eq!(
                    std::fs::read_to_string(dir.path().join("manifest.json")).unwrap(),
                    manifest
                );
            }
        }
    }
}

#[test]
fn migration_honors_request_deadline_for_stalled_space_lookup() {
    use std::net::TcpListener;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("manifest.json"), "{\"objects\": []}").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_kibob"))
        .env_clear()
        .env(
            "KIBANA_URL",
            format!("http://{}", listener.local_addr().unwrap()),
        )
        .env("KIBANA_REQUEST_TIMEOUT", "1")
        .env("KIBANA_CONNECT_TIMEOUT", "1")
        .current_dir(dir.path())
        .args(["--env", "missing.env", "migrate"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut connection = None;
    let status = loop {
        // Accept the request but hold the socket open without a response.
        if connection.is_none() {
            connection = listener.accept().ok();
        }
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("migration ignored the one-second HTTP request deadline");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        connection.is_some(),
        "migration must attempt the space lookup"
    );
    assert!(
        status.success(),
        "optional space lookup timeout must allow local migration"
    );
    assert!(
        dir.path()
            .join("default/manifest/saved_objects.json")
            .exists()
    );
}
