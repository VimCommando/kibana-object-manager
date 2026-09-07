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
    for name in ["KIBANA_REQUEST_TIMEOUT", "KIBANA_CONNECT_TIMEOUT"] {
        for value in ["0", "-1", "1.5", "invalid"] {
            let output = Command::new(env!("CARGO_BIN_EXE_kibob"))
                .env_clear()
                .env("KIBANA_URL", "http://127.0.0.1:1")
                .env(name, value)
                .current_dir(dir.path())
                .args(["--env", "missing.env", "auth"])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(error.contains(&format!("{name} must be a positive integer")));
        }
    }
}
