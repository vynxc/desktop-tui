# Contributing

Thanks for improving Desktop TUI.

## Development setup

Install the dependencies listed in [README.md](README.md), then run:

```bash
make check
make install-no-restart
```

Restart Plasma Shell when you are ready to test the installed applet. Keep
changes focused, add tests for renderer behavior, and update the template
reference when a manifest field changes.

## Pull requests

- Run `make check` before opening the pull request.
- Do not commit personal screenshots, absolute home paths, panel IDs, or
  unlicensed models.
- Keep sample assets small and document their exact source and license in
  `THIRD_PARTY.md`.
- Include before/after media for visible changes.

The CI workflow can be exercised locally with:

```bash
act pull_request -W .github/workflows/ci.yml -j test
```
