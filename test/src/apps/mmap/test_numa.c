#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <time.h>
#include <pthread.h>
#include <sched.h>

#define ARRAY_SIZE (1024 * 1024 * 1024) // 1 GB
#define NUM_THREADS 2
#define NUM_RANDOM_ACCESS 20000000 // 每线程随机访问次数

typedef struct {
	int cpu;
} thread_arg_t;

long long get_ns()
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return (long long)ts.tv_sec * 1000000000LL + ts.tv_nsec;
}

void *thread_init(void *arg)
{
	int *array = malloc(ARRAY_SIZE * sizeof(int));
	if (!array) {
		perror("malloc");
		return NULL;
	}

	thread_arg_t *t = arg;

	// Bind to CPU
	cpu_set_t cpuset;
	CPU_ZERO(&cpuset);
	CPU_SET(t->cpu, &cpuset);
	pthread_setaffinity_np(pthread_self(), sizeof(cpuset), &cpuset);

	volatile long long sum = 0;

	size_t idx;
	for (size_t i = 0; i < NUM_RANDOM_ACCESS; i++) {
		idx = rand() % ARRAY_SIZE;
		sum += *((volatile int *)&array[idx]);
	}

	if (sum == 0xdeadbeef) {
		printf("Impossible\n");
	}

	free(array);
	return NULL;
}

int main()
{
	pthread_t threads[NUM_THREADS];
	thread_arg_t args[NUM_THREADS];

	long long start = get_ns();

	for (int i = 0; i < NUM_THREADS; i++) {
		args[i].cpu = i;
		pthread_create(&threads[i], NULL, thread_init, &args[i]);
	}

	for (int i = 0; i < NUM_THREADS; i++)
		pthread_join(threads[i], NULL);

	long long end = get_ns();
	printf("Time elapsed: %.3f s\n", (end - start) / 1e9);

	return 0;
}
