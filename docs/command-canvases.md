# Command canvases

Status: implemented in Desktop TUI 0.2.0

## Decision

Desktop TUI will support two explicit canvas sources:

1. **Desktop TUI renderer** keeps the existing shared-frame path for efficient
   models and built-in information layouts.
2. **Command output** runs a user-selected executable in the embedded terminal
   and displays its terminal output exactly.

Command execution is widget-instance configuration. It is deliberately not
part of a downloadable visual-template JSON file. Importing a visual template
must never execute a program.

## User experience

The first setting is **Canvas source**:

- **Desktop TUI renderer** reveals the existing template, model, animation, and
  frame-rate controls.
- **Command output** reveals the program, arguments, working directory,
  environment, exit behavior, interval, timeout, and clear-screen controls.

Arguments are entered one per line. Each line becomes one exact process
argument; there is no quote parsing, variable expansion, globbing, pipe, or
shell syntax.

Environment entries use one `NAME=VALUE` pair per line. Empty lines are
ignored. Invalid names stop the command and produce a specific terminal error.

Exit behavior has three choices:

- **Keep output** runs once and leaves the final terminal contents visible.
- **Run periodically** waits for the configured interval after each run,
  optionally clears the canvas, and runs again.
- **Keep running** restarts an exited long-running command with bounded
  exponential backoff.

The existing terminal settings remain shared by both sources. Keyboard input
stays disabled. Mouse input stays disabled unless terminal text selection is
explicitly enabled.

## Process model

```text
Plasma widget
    |
    +-- renderer source --> desktop-tui renderer --> shared frame --> terminal display
    |
    `-- command source  --> desktop-tui command supervisor --> child process --> PTY
```

The applet always starts the installed `desktop-tui` binary. In command mode it
passes `command` as the first argument and supplies a validated specification
through namespaced environment variables. The supervisor launches the selected
program directly with `std::process::Command`.

`/bin/sh -c`, `eval`, and command-string tokenization are not used.

The child inherits the terminal PTY, `TERM=xterm-256color`, and
`COLORTERM=truecolor`. Full-screen TUIs therefore receive terminal resizing and
can use normal ANSI cursor movement. Programs remain responsible for avoiding
opaque background colors when wallpaper transparency is desired.

## Configuration contract

| Key | Type | Default | Meaning |
| --- | --- | --- | --- |
| `canvasSource` | enum | `renderer` | `renderer` or `command` |
| `commandProgram` | string | empty | Executable path or `PATH` name |
| `commandArguments` | string | empty | One argument per line |
| `commandWorkingDirectory` | string | empty | Empty uses the home directory |
| `commandEnvironment` | string | empty | One `NAME=VALUE` pair per line |
| `commandExitBehavior` | enum | `keep-output` | `keep-output`, `interval`, or `restart` |
| `commandIntervalSeconds` | integer | `30` | Delay after an interval run |
| `commandTimeoutSeconds` | integer | `0` | `0` disables the timeout |
| `commandClearBetweenRuns` | boolean | `true` | Clear before a repeated run |

The supervisor clamps intervals to 1–86,400 seconds and timeouts to
0–86,400 seconds. Restart backoff starts at one second, doubles after rapid
failures, and caps at 30 seconds. A command that remains alive for 30 seconds
resets the failure count.

## Failure behavior

Failures are rendered inside the canvas in plain language:

- empty program;
- executable not found or not executable;
- malformed argument JSON from the applet;
- invalid environment entry;
- missing working directory;
- spawn failure;
- timeout;
- non-zero exit status when a repeated command is scheduled.

Repeated failures do not flash the desktop: restart mode backs off and interval
mode observes its configured delay.

Changing any command setting recreates the terminal session. Destroying the
widget destroys the supervisor session and closes its PTY, terminating normal
foreground children with the session.

## Security boundaries

- Command mode is opt-in and visible in the widget settings.
- Visual template files cannot select command mode or provide a command.
- Programs and arguments are passed without a shell.
- Child environment additions are validated and applied only to the child.
- No privilege escalation or password prompting is provided.
- Desktop TUI does not download executables.
- Documentation examples use absolute project paths or ordinary `PATH`
  programs and call out external dependencies.

This protects against accidental shell interpretation. It does not sandbox a
program the user explicitly chooses to run.

## Performance boundaries

The shared-frame renderer remains the recommended path for dense animated
graphics. Command canvases use the terminal PTY because exact terminal behavior
is their purpose. A command that emits data faster than the terminal can parse
will naturally experience PTY backpressure.

Static commands should use **Keep output** or **Run periodically**. Continuous
TUIs should redraw in place and target modest frame rates.

## Test matrix

### Rust unit tests

- parse valid and invalid exit behavior;
- preserve exact argument boundaries;
- reject malformed argument JSON;
- parse environment values containing `=`;
- reject invalid or empty environment names;
- apply interval and timeout bounds;
- calculate and cap restart backoff;
- reset backoff after a stable run;
- validate empty program and missing working directory.

### Rust integration tests

- run `/usr/bin/printf` once and capture exact output;
- preserve an argument containing spaces and punctuation;
- pass validated child environment entries;
- return a useful error for a missing executable;
- terminate a command after its timeout;
- keep shell metacharacters literal rather than interpreting them.

### Applet checks

- QML and KConfig syntax checks;
- every command setting exists in the schema and configuration UI;
- renderer mode still enables shared-frame rendering;
- command mode disables shared-frame rendering;
- renderer and command settings both participate in the restart signature;
- command mode launches only the installed supervisor;
- no `sh -c`, `bash -c`, or `eval` execution path.

### System tests

- `make check`;
- release build;
- native QMLTermWidget build on Qt 6.4 and the current local Qt;
- complete GitHub workflow under `act`;
- install without restarting Plasma;
- verify the existing renderer instance is unchanged;
- run one static ANSI command and one streaming TUI in a disposable instance;
- verify normal pointer, right-click, and middle-click pass-through.

## MPRIS provider

[MPRIS TUI](https://github.com/vynxc/mpris-tui) is the first external command
provider. It runs as a long-lived command canvas, follows the active media
player over the session D-Bus, renders with Ratatui, and requires no keyboard
or mouse input.

Install it with:

```console
cargo install --locked --git https://github.com/vynxc/mpris-tui
```

Then select **Command output** and use:

| Setting | Value |
| --- | --- |
| Program | `mpris-tui` |
| Arguments | `--layout` on one line, `wide` on the next |
| After exit | Keep running |

Choose `hero`, `wide`, `compact`, or `minimal` for each widget instance. Add
`--fps` and a value from `1` to `30` on separate argument lines to cap its
redraw rate.

Its repository, releases, assets, and CI remain independent from Desktop TUI.
