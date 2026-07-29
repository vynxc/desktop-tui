.PHONY: build check doctor install install-no-restart uninstall

build:
	cargo build --package desktop-tui --release --locked

check:
	./scripts/check.sh

doctor:
	./scripts/doctor.sh

install:
	./scripts/install.sh

install-no-restart:
	./scripts/install.sh --no-restart

uninstall:
	./scripts/uninstall.sh
