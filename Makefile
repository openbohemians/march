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
	@for test in tests/*.march2; do \
		if [ -f "$$test" ]; then \
			echo "Testing: $$test"; \
			./target/release/march2 < "$$test"; \
			echo ""; \
		fi; \
	done

# Run tests with output (for debugging)
test-verbose: build
	@echo "Running all tests (verbose)..."
	@echo ""
	@for test in tests/*.march2; do \
		if [ -f "$$test" ]; then \
			echo "========================================"; \
			echo "Testing: $$test"; \
			echo "========================================"; \
			./target/release/march2 < "$$test"; \
			echo ""; \
		fi; \
	done

# Run a specific test
test-one: build
	@if [ -z "$(TEST)" ]; then \
		echo "Usage: make test-one TEST=tests/test_simple.march2"; \
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
