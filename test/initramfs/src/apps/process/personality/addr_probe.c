// SPDX-License-Identifier: MPL-2.0

#include <stdint.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <unistd.h>

#define GET_PERSONALITY 0xffffffffu

static long get_personality(void)
{
	return syscall(SYS_personality, GET_PERSONALITY);
}

int main(void)
{
	int stack_var = 0;
	void *program_break = sbrk(0);
	long personality = get_personality();

	if (personality < 0 || program_break == (void *)-1)
		return 1;

	printf("stack=%#lx brk=%#lx text=%#lx personality=%#lx\n",
	       (unsigned long)(uintptr_t)&stack_var,
	       (unsigned long)(uintptr_t)program_break,
	       (unsigned long)(uintptr_t)&main, (unsigned long)personality);
	return 0;
}
