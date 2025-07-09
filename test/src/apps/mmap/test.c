#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <pthread.h>
#include <string.h>
#include <sys/wait.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sched.h>
#include <stdint.h>
#include <errno.h>

#define MAX(a, b) ((a) > (b) ? (a) : (b))

typedef struct {
	pid_t tid;
	volatile uint64_t counter;
} worker_data_t;

int cpu_count;
worker_data_t *workers_data;

void *worker(void *arg)
{
	worker_data_t *data = (worker_data_t *)arg;
	data->tid = syscall(SYS_gettid);

	// printf("worker tid: %d\n", data->tid);

	while (1) {
		data->counter++;
		sched_yield();
	}

	return NULL;
}

void spawn_threads()
{
	// printf("cpu_count: %d\n", cpu_count);
	pthread_t *threads = malloc(sizeof(pthread_t) * cpu_count);
	if (!threads) {
		perror("malloc");
		exit(1);
	}

	for (int i = 0; i < cpu_count; i++) {
		if (pthread_create(&threads[i], NULL, worker,
				   &workers_data[i]) != 0) {
			perror("pthread_create");
			exit(1);
		}
		// printf("pthread_create tid: %d\n", workers_data[i].tid);
	}

	// 主线程不等待 threads，这些线程在 exit_group 后应该全部终止
}

void check_counters()
{
	worker_data_t *old_data = malloc(sizeof(worker_data_t) * cpu_count);
	if (!old_data) {
		perror("malloc");
		exit(1);
	}

	memcpy(old_data, workers_data, sizeof(worker_data_t) * cpu_count);
	usleep(100000); // 100ms
	int all_dead = 1;

	for (int i = 0; i < cpu_count; i++) {
		if (old_data[i].counter != workers_data[i].counter) {
			printf("❌ Thread[%d] is still running (counter changed: %lu -> %lu)\n",
			       i, old_data[i].counter, workers_data[i].counter);
			all_dead = 0;
		}
	}

	if (all_dead) {
		printf("✅ All threads are stopped: counters did not change\n");
	}

	free(old_data);
}

int main()
{
	cpu_count = MAX(2, sysconf(_SC_NPROCESSORS_ONLN));

	size_t map_size = sizeof(worker_data_t) * cpu_count;
	workers_data = mmap(NULL, map_size, PROT_READ | PROT_WRITE,
			    MAP_SHARED | MAP_ANONYMOUS, -1, 0);
	if (workers_data == MAP_FAILED) {
		perror("mmap");
		exit(1);
	}

	pid_t pid = fork();
	if (pid < 0) {
		perror("fork");
		exit(1);
	}

	if (pid == 0) {
		// 子进程
		spawn_threads();

		// 退出整个线程组，退出码为 4
		syscall(SYS_exit_group, 4);

		// 不应走到这里
		_exit(1);
	}

	// 父进程
	int status;
	waitpid(pid, &status, 0);

	if (WIFEXITED(status) && WEXITSTATUS(status) == 4) {
		printf("✅ exit_group() succeeded with code 4\n");
	} else {
		printf("❌ exit_group() failed, status = 0x%x\n", status);
	}

	check_counters();

	munmap(workers_data, map_size);
	return 0;
}
