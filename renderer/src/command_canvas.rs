use std::{
    env,
    fmt::{self, Display, Formatter},
    io::{self, Write},
    path::PathBuf,
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

const PROGRAM_ENV: &str = "DESKTOP_TUI_COMMAND_PROGRAM";
const ARGUMENTS_ENV: &str = "DESKTOP_TUI_COMMAND_ARGUMENTS_JSON";
const WORKING_DIRECTORY_ENV: &str = "DESKTOP_TUI_COMMAND_WORKING_DIRECTORY";
const ENVIRONMENT_ENV: &str = "DESKTOP_TUI_COMMAND_ENVIRONMENT";
const EXIT_BEHAVIOR_ENV: &str = "DESKTOP_TUI_COMMAND_EXIT_BEHAVIOR";
const INTERVAL_ENV: &str = "DESKTOP_TUI_COMMAND_INTERVAL_SECONDS";
const TIMEOUT_ENV: &str = "DESKTOP_TUI_COMMAND_TIMEOUT_SECONDS";
const CLEAR_ENV: &str = "DESKTOP_TUI_COMMAND_CLEAR_BETWEEN_RUNS";

const DEFAULT_INTERVAL_SECONDS: u64 = 30;
const MAX_DELAY_SECONDS: u64 = 86_400;
const STABLE_RUN_SECONDS: u64 = 30;
const MAX_RESTART_DELAY_SECONDS: u64 = 30;
const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExitBehavior {
    KeepOutput,
    Interval,
    Restart,
}

#[derive(Debug)]
struct CommandCanvasSettings {
    program: String,
    arguments: Vec<String>,
    working_directory: Option<PathBuf>,
    environment: Vec<(String, String)>,
    exit_behavior: ExitBehavior,
    interval: Duration,
    timeout: Option<Duration>,
    clear_between_runs: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandCanvasError(String);

impl Display for CommandCanvasError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CommandCanvasError {}

pub(crate) fn requested() -> bool {
    env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "command")
}

pub(crate) fn run() -> Result<(), Box<dyn std::error::Error>> {
    env::remove_var("NO_COLOR");
    env::set_var("TERM", "xterm-256color");
    env::set_var("COLORTERM", "truecolor");

    let settings = CommandCanvasSettings::load()?;
    supervise(&settings).map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
}

impl CommandCanvasSettings {
    fn load() -> Result<Self, CommandCanvasError> {
        let program = env::var(PROGRAM_ENV).unwrap_or_default();
        let program = program.trim();
        if program.is_empty() {
            return Err(CommandCanvasError(
                "command canvas has no program; configure one in the widget settings".into(),
            ));
        }

        let arguments = parse_arguments(&env::var(ARGUMENTS_ENV).unwrap_or_else(|_| "[]".into()))?;
        let working_directory =
            optional_directory(&env::var(WORKING_DIRECTORY_ENV).unwrap_or_default())?;
        let environment = parse_environment(&env::var(ENVIRONMENT_ENV).unwrap_or_default())?;
        let exit_behavior = parse_exit_behavior(&env::var(EXIT_BEHAVIOR_ENV).unwrap_or_default())?;
        let interval = Duration::from_secs(bounded_seconds(
            env::var(INTERVAL_ENV).ok().as_deref(),
            DEFAULT_INTERVAL_SECONDS,
            1,
        ));
        let timeout_seconds = bounded_seconds(env::var(TIMEOUT_ENV).ok().as_deref(), 0, 0);
        let timeout = (timeout_seconds > 0).then(|| Duration::from_secs(timeout_seconds));
        let clear_between_runs = parse_bool(env::var(CLEAR_ENV).ok().as_deref(), true);

        Ok(Self {
            program: program.into(),
            arguments,
            working_directory,
            environment,
            exit_behavior,
            interval,
            timeout,
            clear_between_runs,
        })
    }
}

fn supervise(settings: &CommandCanvasSettings) -> Result<(), CommandCanvasError> {
    let mut rapid_failures = 0_u32;
    let mut first_run = true;

    loop {
        if !first_run && settings.clear_between_runs {
            clear_terminal()?;
        }
        first_run = false;

        let started = Instant::now();
        let result = run_child(settings);
        let elapsed = started.elapsed();

        match settings.exit_behavior {
            ExitBehavior::KeepOutput => return finish_once(result),
            ExitBehavior::Interval => {
                report_result(&result);
                thread::sleep(settings.interval);
            }
            ExitBehavior::Restart => {
                report_result(&result);
                rapid_failures = next_failure_count(rapid_failures, elapsed);
                let delay = restart_delay(rapid_failures);
                eprintln!(
                    "desktop-tui: command exited; restarting in {} second{}",
                    delay.as_secs(),
                    if delay.as_secs() == 1 { "" } else { "s" }
                );
                thread::sleep(delay);
            }
        }
    }
}

fn run_child(settings: &CommandCanvasSettings) -> Result<ExitStatus, CommandCanvasError> {
    let mut command = Command::new(&settings.program);
    command.args(&settings.arguments);
    if let Some(directory) = &settings.working_directory {
        command.current_dir(directory);
    }
    command.envs(
        settings
            .environment
            .iter()
            .map(|(name, value)| (name, value)),
    );

    let mut child = command.spawn().map_err(|error| {
        CommandCanvasError(format!(
            "could not start command `{}`: {error}",
            settings.program
        ))
    })?;

    let Some(timeout) = settings.timeout else {
        return child
            .wait()
            .map_err(|error| CommandCanvasError(format!("could not wait for command: {error}")));
    };

    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            CommandCanvasError(format!("could not inspect command status: {error}"))
        })? {
            return Ok(status);
        }
        if started.elapsed() >= timeout {
            child.kill().map_err(|error| {
                CommandCanvasError(format!(
                    "command exceeded its {} second timeout and could not be stopped: {error}",
                    timeout.as_secs()
                ))
            })?;
            let _ = child.wait();
            return Err(CommandCanvasError(format!(
                "command exceeded its {} second timeout",
                timeout.as_secs()
            )));
        }
        thread::sleep(CHILD_POLL_INTERVAL);
    }
}

fn finish_once(result: Result<ExitStatus, CommandCanvasError>) -> Result<(), CommandCanvasError> {
    match result {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(CommandCanvasError(format!(
            "command exited with {}",
            exit_description(status)
        ))),
        Err(error) => Err(error),
    }
}

fn report_result(result: &Result<ExitStatus, CommandCanvasError>) {
    match result {
        Ok(status) if status.success() => {}
        Ok(status) => eprintln!(
            "desktop-tui: command exited with {}",
            exit_description(*status)
        ),
        Err(error) => eprintln!("desktop-tui: {error}"),
    }
}

fn exit_description(status: ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "a signal".into(), |code| format!("status {code}"))
}

fn clear_terminal() -> Result<(), CommandCanvasError> {
    io::stdout()
        .write_all(b"\x1b[2J\x1b[H")
        .and_then(|_| io::stdout().flush())
        .map_err(|error| CommandCanvasError(format!("could not clear the terminal: {error}")))
}

fn parse_arguments(value: &str) -> Result<Vec<String>, CommandCanvasError> {
    serde_json::from_str(value).map_err(|error| {
        CommandCanvasError(format!(
            "command arguments could not be decoded as a JSON array: {error}"
        ))
    })
}

fn parse_environment(value: &str) -> Result<Vec<(String, String)>, CommandCanvasError> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (name, value) = line.split_once('=').ok_or_else(|| {
                CommandCanvasError(format!(
                    "invalid environment entry `{line}`; expected NAME=VALUE"
                ))
            })?;
            if !valid_environment_name(name) {
                return Err(CommandCanvasError(format!(
                    "invalid environment variable name `{name}`"
                )));
            }
            Ok((name.into(), value.into()))
        })
        .collect()
}

fn valid_environment_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some('_' | 'a'..='z' | 'A'..='Z'))
        && characters.all(|character| matches!(character, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9'))
}

fn optional_directory(value: &str) -> Result<Option<PathBuf>, CommandCanvasError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(value);
    if !path.is_dir() {
        return Err(CommandCanvasError(format!(
            "command working directory does not exist: {}",
            path.display()
        )));
    }
    Ok(Some(path))
}

fn parse_exit_behavior(value: &str) -> Result<ExitBehavior, CommandCanvasError> {
    match value.trim() {
        "" | "keep-output" => Ok(ExitBehavior::KeepOutput),
        "interval" => Ok(ExitBehavior::Interval),
        "restart" => Ok(ExitBehavior::Restart),
        value => Err(CommandCanvasError(format!(
            "unknown command exit behavior `{value}`"
        ))),
    }
}

fn bounded_seconds(value: Option<&str>, default: u64, minimum: u64) -> u64 {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
        .clamp(minimum, MAX_DELAY_SECONDS)
}

fn parse_bool(value: Option<&str>, default: bool) -> bool {
    value.map_or(default, |value| {
        !matches!(value.trim(), "0" | "false" | "no" | "off")
    })
}

fn next_failure_count(current: u32, elapsed: Duration) -> u32 {
    if elapsed >= Duration::from_secs(STABLE_RUN_SECONDS) {
        0
    } else {
        current.saturating_add(1)
    }
}

fn restart_delay(failures: u32) -> Duration {
    let exponent = failures.saturating_sub(1).min(5);
    Duration::from_secs(
        1_u64
            .checked_shl(exponent)
            .unwrap_or(MAX_RESTART_DELAY_SECONDS)
            .min(MAX_RESTART_DELAY_SECONDS),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_argument_boundaries() {
        let arguments = parse_arguments(r#"["one value","$HOME","*.rs",""]"#).unwrap();
        assert_eq!(arguments, ["one value", "$HOME", "*.rs", ""]);
    }

    #[test]
    fn rejects_non_array_arguments() {
        let error = parse_arguments(r#"{"argument":"value"}"#).unwrap_err();
        assert!(error.to_string().contains("JSON array"));
    }

    #[test]
    fn parses_environment_values_containing_equals() {
        let environment = parse_environment("TOKEN=left=right\nEMPTY=\n").unwrap();
        assert_eq!(
            environment,
            [
                ("TOKEN".into(), "left=right".into()),
                ("EMPTY".into(), String::new())
            ]
        );
    }

    #[test]
    fn rejects_invalid_environment_names() {
        assert!(parse_environment("GOOD=value").is_ok());
        assert!(parse_environment("9BAD=value").is_err());
        assert!(parse_environment("ALSO-BAD=value").is_err());
        assert!(parse_environment("MISSING_VALUE").is_err());
    }

    #[test]
    fn parses_exit_behaviors() {
        assert_eq!(
            parse_exit_behavior("keep-output").unwrap(),
            ExitBehavior::KeepOutput
        );
        assert_eq!(
            parse_exit_behavior("interval").unwrap(),
            ExitBehavior::Interval
        );
        assert_eq!(
            parse_exit_behavior("restart").unwrap(),
            ExitBehavior::Restart
        );
        assert!(parse_exit_behavior("loop-forever").is_err());
    }

    #[test]
    fn bounds_delays() {
        assert_eq!(bounded_seconds(Some("0"), 30, 1), 1);
        assert_eq!(bounded_seconds(Some("999999"), 30, 1), 86_400);
        assert_eq!(bounded_seconds(Some("invalid"), 30, 1), 30);
        assert_eq!(bounded_seconds(None, 0, 0), 0);
    }

    #[test]
    fn restart_backoff_is_bounded() {
        assert_eq!(restart_delay(1), Duration::from_secs(1));
        assert_eq!(restart_delay(2), Duration::from_secs(2));
        assert_eq!(restart_delay(5), Duration::from_secs(16));
        assert_eq!(restart_delay(6), Duration::from_secs(30));
        assert_eq!(restart_delay(100), Duration::from_secs(30));
    }

    #[test]
    fn stable_run_resets_restart_failures() {
        assert_eq!(next_failure_count(4, Duration::from_secs(2)), 5);
        assert_eq!(
            next_failure_count(4, Duration::from_secs(STABLE_RUN_SECONDS)),
            0
        );
    }
}
