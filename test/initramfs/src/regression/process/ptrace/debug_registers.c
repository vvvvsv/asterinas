// SPDX-License-Identifier: MPL-2.0

#define _GNU_SOURCE

#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/ptrace.h>
#include <sys/user.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../../common/test.h"
#include "../../common/yama_ptrace_scope.h"

#ifdef __x86_64__
#include <asm/debugreg.h>

#define DR_RW_RESERVED 0x2UL

static unsigned long debugreg_offset(int reg_num)
{
	return offsetof(struct user, u_debugreg[0]) +
	       reg_num * sizeof(unsigned long);
}

__attribute__((noinline)) static void break_here(void)
{
	asm volatile("nop" ::: "memory");
}

FN_TEST(read_write_debug_registers)
{
	SKIP_TEST_IF(read_yama_scope() == YAMA_SCOPE_NO_ATTACH);

	pid_t pid = TEST_SUCC(fork());
	if (pid == 0) {
		CHECK(ptrace(PTRACE_TRACEME, 0, 0, 0));
		CHECK(raise(SIGSTOP));
		break_here();
		_exit(0);
	}

	int status = 0;
	TEST_RES(waitpid(pid, &status, 0), _ret == pid && WIFSTOPPED(status) &&
						   WSTOPSIG(status) == SIGSTOP);

	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(0), 0),
		 _ret == 0);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(1), 0),
		 _ret == 0);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(2), 0),
		 _ret == 0);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(3), 0),
		 _ret == 0);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(DR_STATUS), 0),
		 _ret == DR6_RESERVED);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(DR_CONTROL), 0),
		 _ret == 0);

	const unsigned long dr0_value = (uintptr_t)break_here;
	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(0), dr0_value));
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(0), 0),
		 _ret == dr0_value);

	const unsigned long dr1_value = dr0_value + 0x10;
	const unsigned long dr2_value = dr0_value + 0x20;
	const unsigned long dr3_value = dr0_value + 0x30;
	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(1), dr1_value));
	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(2), dr2_value));
	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(3), dr3_value));
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(1), 0),
		 _ret == dr1_value);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(2), 0),
		 _ret == dr2_value);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(3), 0),
		 _ret == dr3_value);

	const unsigned long dr6_value = DR6_RESERVED | DR_STEP | DR_TRAP1;
	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(DR_STATUS),
			 dr6_value));
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(DR_STATUS), 0),
		 _ret == dr6_value);

	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(4), 0),
		 _ret == 0);
	TEST_ERRNO(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(4), dr0_value),
		   EIO);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(5), 0),
		 _ret == 0);
	TEST_ERRNO(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(5), dr0_value),
		   EIO);

	TEST_ERRNO(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(1),
			  (uintptr_t)-1),
		   EINVAL);

	const unsigned long invalid_dr7 = DR_LOCAL_ENABLE |
					  (DR_RW_RESERVED << DR_CONTROL_SHIFT);
	TEST_ERRNO(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(DR_CONTROL),
			  invalid_dr7),
		   EINVAL);

	const unsigned long execute_dr7 = DR_LOCAL_ENABLE |
					  (DR_RW_EXECUTE << DR_CONTROL_SHIFT);
	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(DR_CONTROL),
			 execute_dr7));
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(DR_CONTROL), 0),
		 _ret == execute_dr7);

	TEST_SUCC(ptrace(PTRACE_CONT, pid, 0, 0));
	TEST_RES(waitpid(pid, &status, 0), _ret == pid && WIFSTOPPED(status) &&
						   WSTOPSIG(status) == SIGTRAP);
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(DR_STATUS), 0),
		 (_ret & DR_TRAP0) != 0);

	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(DR_STATUS),
			 DR6_RESERVED));
	TEST_SUCC(ptrace(PTRACE_POKEUSER, pid, debugreg_offset(DR_CONTROL), 0));
	TEST_RES(ptrace(PTRACE_PEEKUSER, pid, debugreg_offset(DR_CONTROL), 0),
		 _ret == 0);

	TEST_SUCC(ptrace(PTRACE_CONT, pid, 0, 0));
	TEST_RES(waitpid(pid, &status, 0),
		 _ret == pid && WIFEXITED(status) && WEXITSTATUS(status) == 0);
}
END_TEST()

#else

int main(void)
{
	return 0;
}

#endif
