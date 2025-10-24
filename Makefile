# March Language Build System
# Builds assembly primitives, VM, runtime, and test program

# Assembler and compiler
NASM = nasm
CC = gcc
LD = gcc
CARGO = cargo

# Flags
NASMFLAGS = -f elf64 -g -F dwarf
NASMFLAGS_PIC = -f elf64 -g -F dwarf -DPIC
CFLAGS = -Wall -Wextra -g -O0
LDFLAGS = -no-pie

# Directories
KERNEL_DIR = kernel/x86-64
RUNTIME_DIR = runtime
BUILD_DIR = build

# Rust runtime library
RUNTIME_LIB = $(RUNTIME_DIR)/target/release/libmarch_runtime.a

# Source files
ASM_SOURCES = $(filter-out $(KERNEL_DIR)/vm.asm, $(wildcard $(KERNEL_DIR)/*.asm))
ASM_OBJECTS = $(patsubst $(KERNEL_DIR)/%.asm,$(BUILD_DIR)/%.o,$(ASM_SOURCES))
ASM_PIC_OBJECTS = $(patsubst $(KERNEL_DIR)/%.asm,$(BUILD_DIR)/%-pic.o,$(ASM_SOURCES))

# Targets
TEST_PROGRAM = $(BUILD_DIR)/test_vm
VM_LIB = $(BUILD_DIR)/libmarch_vm.a

.PHONY: all clean test dirs runtime clean-runtime vmlib

all: dirs runtime $(TEST_PROGRAM) vmlib

# Build VM static library for OCaml FFI (with PIC)
vmlib: $(VM_LIB)

$(VM_LIB): $(ASM_PIC_OBJECTS)
	@echo "Creating VM static library (PIC)..."
	@ar rcs $@ $(ASM_PIC_OBJECTS)
	@echo "Built $@"

dirs:
	@mkdir -p $(BUILD_DIR)

# Compile assembly files
$(BUILD_DIR)/%.o: $(KERNEL_DIR)/%.asm
	@echo "Assembling $<..."
	@$(NASM) $(NASMFLAGS) $< -o $@

# Compile assembly files with PIC (for OCaml FFI)
$(BUILD_DIR)/%-pic.o: $(KERNEL_DIR)/%.asm
	@echo "Assembling $< (PIC)..."
	@$(NASM) $(NASMFLAGS_PIC) $< -o $@

# Compile C test program
$(BUILD_DIR)/test_vm.o: test_vm.c
	@echo "Compiling $<..."
	@$(CC) $(CFLAGS) -c $< -o $@

# Build Rust runtime
runtime: $(RUNTIME_LIB)

$(RUNTIME_LIB):
	@echo "Building Rust runtime..."
	@cd $(RUNTIME_DIR) && $(CARGO) build --release

# Link everything together (without runtime for now - not needed for VM tests)
$(TEST_PROGRAM): $(BUILD_DIR)/test_vm.o $(ASM_OBJECTS)
	@echo "Linking $@..."
	@$(LD) $(LDFLAGS) $(BUILD_DIR)/test_vm.o $(ASM_OBJECTS) -o $@
	@echo "Build complete: $@"

# Run tests
test: $(TEST_PROGRAM)
	@echo ""
	@echo "Running tests..."
	@echo ""
	@./$(TEST_PROGRAM)

# Clean build artifacts
clean: clean-runtime
	@echo "Cleaning build directory..."
	@rm -rf $(BUILD_DIR)

# Clean Rust artifacts
clean-runtime:
	@echo "Cleaning Rust runtime..."
	@cd $(RUNTIME_DIR) && $(CARGO) clean

# Show what will be built
info:
	@echo "Assembly sources:"
	@echo "$(ASM_SOURCES)" | tr ' ' '\n'
	@echo ""
	@echo "Object files:"
	@echo "$(ASM_OBJECTS)" | tr ' ' '\n'
