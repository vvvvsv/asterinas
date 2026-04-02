// SPDX-License-Identifier: MPL-2.0

#include "../../common/test.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

#define GET_PERSONALITY 0xffffffffu
#define ADDR_NO_RANDOMIZE 0x0040000u
#define DEFAULT_PERSONALITY 0u
#define EXE_PATH_MAX 512

struct probe_result {
	unsigned long stack;
	unsigned long program_break;
	unsigned long text;
	unsigned long personality;
};

static long do_personality(unsigned long personality)
{
	return syscall(SYS_personality, personality);
}

static void reset_personality(void)
{
	CHECK(do_personality(DEFAULT_PERSONALITY));
}

static void build_probe_path(char *path, size_t path_len)
{
	ssize_t exe_len;
	char *file_name;

	exe_len = CHECK_WITH(readlink("/proc/self/exe", path, path_len - 1),
			     _ret > 0 && _ret < (ssize_t)(path_len - 1));
	path[exe_len] = '\0';

	file_name = strrchr(path, '/');
	CHECK_WITH(file_name != NULL, _ret);
	strcpy(file_name + 1, "addr_probe");
}

static void run_probe(struct probe_result *result)
{
	char probe_path[EXE_PATH_MAX];
	char output[256];
	ssize_t total_len = 0;
	int pipe_fds[2];
	int status;
	pid_t child_pid;

	build_probe_path(probe_path, sizeof(probe_path));
	CHECK(pipe(pipe_fds));

	child_pid = CHECK(fork());
	if (child_pid == 0) {
		char *const argv[] = { "addr_probe", NULL };
		char *const envp[] = { NULL };

		close(pipe_fds[0]);
		CHECK(dup2(pipe_fds[1], STDOUT_FILENO));
		close(pipe_fds[1]);
		execve(probe_path, argv, envp);
		_exit(127);
	}

	close(pipe_fds[1]);
	while (total_len < (ssize_t)(sizeof(output) - 1)) {
		ssize_t read_len =
			read(pipe_fds[0], output + total_len, sizeof(output) - 1 - total_len);
		if (read_len < 0) {
			perror("read");
			exit(EXIT_FAILURE);
		}
		if (read_len == 0)
			break;

		total_len += read_len;
	}
	output[total_len] = '\0';

	CHECK(close(pipe_fds[0]));
	CHECK(waitpid(child_pid, &status, 0));
	CHECK_WITH(status, WIFEXITED(status) && WEXITSTATUS(status) == 0);
	CHECK_WITH(sscanf(output,
			  "stack=%lx brk=%lx text=%lx personality=%lx",
			  &result->stack,
			  &result->program_break,
			  &result->text,
			  &result->personality),
		   _ret == 4);
}

static bool layout_differs(const struct probe_result *left,
			   const struct probe_result *right)
{
	return left->stack != right->stack ||
	       left->program_break != right->program_break || left->text != right->text;
}

static bool layout_matches(const struct probe_result *left,
			   const struct probe_result *right)
{
	return !layout_differs(left, right);
}

FN_TEST(can_get_and_set_personality)
{
	reset_personality();

	TEST_RES(do_personality(GET_PERSONALITY), _ret == DEFAULT_PERSONALITY);
	TEST_RES(do_personality(ADDR_NO_RANDOMIZE), _ret == DEFAULT_PERSONALITY);
	TEST_RES(do_personality(GET_PERSONALITY), _ret == ADDR_NO_RANDOMIZE);
	TEST_RES(do_personality(DEFAULT_PERSONALITY), _ret == ADDR_NO_RANDOMIZE);
	TEST_RES(do_personality(GET_PERSONALITY), _ret == DEFAULT_PERSONALITY);
}
END_TEST()

FN_TEST(personality_is_inherited_across_fork)
{
	int status;
	pid_t child_pid;

	reset_personality();
	CHECK(do_personality(ADDR_NO_RANDOMIZE));

	child_pid = CHECK(fork());
	if (child_pid == 0) {
		long personality = do_personality(GET_PERSONALITY);
		_exit(personality == ADDR_NO_RANDOMIZE ? 0 : 1);
	}

	TEST_RES(waitpid(child_pid, &status, 0),
		 _ret == child_pid && WIFEXITED(status) && WEXITSTATUS(status) == 0);
	reset_personality();
}
END_TEST()

FN_TEST(addr_no_randomize_makes_exec_layout_stable)
{
	enum {
		NR_RANDOMIZED_RUNS = 8,
		NR_DETERMINISTIC_RUNS = 4,
	};

	struct probe_result probe_results[NR_RANDOMIZED_RUNS];
	bool any_layout_differs = false;

	reset_personality();
	for (size_t i = 0; i < NR_RANDOMIZED_RUNS; i++)
		run_probe(&probe_results[i]);

	for (size_t i = 1; i < NR_RANDOMIZED_RUNS; i++) {
		if (layout_differs(&probe_results[0], &probe_results[i])) {
			any_layout_differs = true;
			break;
		}
	}

	TEST_RES(any_layout_differs, _ret);

	CHECK(do_personality(ADDR_NO_RANDOMIZE));
	for (size_t i = 0; i < NR_DETERMINISTIC_RUNS; i++) {
		run_probe(&probe_results[i]);
		TEST_RES(probe_results[i].personality, _ret == ADDR_NO_RANDOMIZE);
	}

	for (size_t i = 1; i < NR_DETERMINISTIC_RUNS; i++)
		TEST_RES(layout_matches(&probe_results[0], &probe_results[i]), _ret);

	reset_personality();
}
END_TEST()
