.PHONY: all build test clean help

# Default target
all: build

# Build the interpreter
build:
	@echo "Building march2..."
	@cargo build --release

# Run all tests
test: build
	@echo "Running all tests..."
	@echo ""
	@failed=0; \
	for test in tests/*.forth; do \
		echo "Testing: $$test"; \
		if timeout 5s ./target/release/march2 < "$$test" > /dev/null 2>&1; then \
			echo "  ✓ PASS"; \
		else \
			echo "  ✗ FAIL"; \
			failed=$$((failed + 1)); \
		fi; \
		echo ""; \
	done; \
	if [ $$failed -eq 0 ]; then \
		echo "All tests passed!"; \
	else \
		echo "$$failed test(s) failed"; \
		exit 1; \
	fi

# Run tests with output (for debugging)
test-verbose: build
	@echo "Running all tests (verbose)..."
	@echo ""
	@for test in tests/*.forth; do \
		echo "========================================"; \
		echo "Testing: $$test"; \
		echo "========================================"; \
		timeout 5s ./target/release/march2 < "$$test"; \
		echo ""; \
	done

# Run a specific test
test-one: build
	@if [ -z "$(TEST)" ]; then \
		echo "Usage: make test-one TEST=tests/test_foo.forth"; \
		exit 1; \
	fi
	@echo "Running $(TEST)..."
	@./target/release/march2 < "$(TEST)"

# Clean build artifacts
clean:
	@echo "Cleaning..."
	@cargo clean

# Show help
help:
	@echo "March2 Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  make              - Build the interpreter"
	@echo "  make build        - Build the interpreter"
	@echo "  make test         - Run all tests (silent)"
	@echo "  make test-verbose - Run all tests with output"
	@echo "  make test-one TEST=<file> - Run a specific test"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make help         - Show this help"
