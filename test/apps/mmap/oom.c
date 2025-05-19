#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <unistd.h>
#include <string.h>
#include <errno.h>

#define GB (1024L * 1024L * 1024L)
#define PAGE_SIZE 4096

int main(int argc, char *argv[])
{
	if (argc != 2) {
		fprintf(stderr, "Usage: %s <size_in_GB>\n", argv[0]);
		return 1;
	}

	char *endptr;
	long input_gb = strtol(argv[1], &endptr, 10);
	if (*endptr != '\0' || input_gb <= 0) {
		fprintf(stderr,
			"Invalid input: '%s'. Please provide a positive integer.\n",
			argv[1]);
		return 1;
	}

	size_t total = input_gb * GB;
	size_t touched = 0;
	void *addr;

	addr = mmap(NULL, total, PROT_READ | PROT_WRITE,
		    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	if (addr == MAP_FAILED) {
		perror("mmap failed");
		return 1;
	}

	printf("PID: %d. mmap %ld GB done. Triggering page faults...\n",
	       getpid(), input_gb);

	char *p = (char *)addr;
	for (size_t i = 0; i < total; i += PAGE_SIZE) {
		p[i] = 1;
		touched += PAGE_SIZE;

		if (touched % GB == 0) {
			printf("Triggered page faults for %ld GB\n",
			       touched / GB);
		}
	}

	printf("Done triggering page faults for %ld GB.\n", input_gb);

	while (1)
		pause();

	munmap(addr, total);
	return 0;
}
