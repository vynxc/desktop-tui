use std::process::Command;

fn command_canvas(program: &str, arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_desktop-tui"));
    command
        .arg("command")
        .env("DESKTOP_TUI_COMMAND_PROGRAM", program)
        .env(
            "DESKTOP_TUI_COMMAND_ARGUMENTS_JSON",
            serde_json::to_string(arguments).unwrap(),
        )
        .env("DESKTOP_TUI_COMMAND_EXIT_BEHAVIOR", "keep-output")
        .env("DESKTOP_TUI_COMMAND_TIMEOUT_SECONDS", "0");
    command
}

#[test]
fn renders_exact_command_output() {
    let output = command_canvas("/usr/bin/printf", &["hello %s", "desktop tui"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"hello desktop tui");
    assert!(output.stderr.is_empty());
}

#[test]
fn keeps_shell_metacharacters_literal() {
    let output = command_canvas("/usr/bin/printf", &["%s", "$(echo not-a-shell) | *.rs"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(output.stdout, b"$(echo not-a-shell) | *.rs");
}

#[test]
fn passes_validated_child_environment() {
    let output = command_canvas("/usr/bin/env", &[])
        .env("DESKTOP_TUI_COMMAND_ENVIRONMENT", "DTUI_TEST=left=right")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.lines().any(|line| line == "DTUI_TEST=left=right"));
}

#[test]
fn honors_the_working_directory() {
    let output = command_canvas("/usr/bin/pwd", &[])
        .env("DESKTOP_TUI_COMMAND_WORKING_DIRECTORY", "/tmp")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "/tmp");
}

#[test]
fn reports_a_missing_executable() {
    let output = command_canvas("/definitely/not/a/desktop-tui-command", &[])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("could not start command"));
}

#[test]
fn terminates_a_command_after_its_timeout() {
    let output = command_canvas("/usr/bin/sleep", &["2"])
        .env("DESKTOP_TUI_COMMAND_TIMEOUT_SECONDS", "1")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("exceeded its 1 second timeout"));
}
