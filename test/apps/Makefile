# SPDX-License-Identifier: MPL-2.0

# Prevent the implicit rules from compiling ".c" or ".s" files automatically.
MAKEFLAGS += --no-builtin-rules

MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
CUR_DIR := $(patsubst %/,%,$(dir $(MKFILE_PATH)))

INITRAMFS ?= $(CUR_DIR)/../build/initramfs
TEST_BUILD_DIR ?= $(INITRAMFS)/test

# These test apps are sorted by name
TEST_APPS := \
	alarm \
	capability \
	clone3 \
	cpu_affinity \
	epoll \
	eventfd2 \
	execve \
	exit \
	fdatasync \
	file_io \
	fork \
	fork_c \
	getcpu \
	getpid \
	hello_c \
	hello_pie \
	hello_world \
	itimer \
	mmap \
	mongoose \
	network \
	pipe \
	prctl \
	pthread \
	pty \
	sched \
	shm \
	signal_c \
	vsock \
	rvault_tests \

# The C head and source files of all the apps, excluding the downloaded mongoose files
C_SOURCES := \
	$(shell find . -type f \( -name "*.c" -or -name "*.h" \) \
		! -name "mongoose.c" ! -name "mongoose.h")

.PHONY: all
all: $(TEST_APPS) scripts hello rvault

.PHONY: $(TEST_APPS)
$(TEST_APPS):
	@make --no-print-directory -C $@

.PHONY: format
format:
	@echo "Fixing code format for general tests..."
	@clang-format -i $(C_SOURCES)

.PHONY: check
check:
	@echo "Checking code format for general tests..."
	@clang-format --dry-run --Werror $(C_SOURCES)

$(TEST_BUILD_DIR):
	@mkdir -p $@

.PHONY: scripts
scripts: | $(TEST_BUILD_DIR)
	@make --no-print-directory BUILD_DIR=$(TEST_BUILD_DIR) -C scripts

.PHONY: hello
hello:
	cargo build --release --manifest-path hello/Cargo.toml
	cp hello/target/x86_64-unknown-linux-gnu/release/hello $(TEST_BUILD_DIR)

.PHONY: rvault
rvault:
	cargo build --manifest-path RustyVault/Cargo.toml
	mkdir -p $(TEST_BUILD_DIR)/rvault/work_dir
	cp RustyVault/target/debug/rvault $(TEST_BUILD_DIR)/rvault/
	cp RustyVault/rvault.hcl $(TEST_BUILD_DIR)/rvault/
	cp /lib/x86_64-linux-gnu/libssl.so.3 $(INITRAMFS)/lib/x86_64-linux-gnu/libssl.so.3