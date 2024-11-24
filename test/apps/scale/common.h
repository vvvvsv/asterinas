#define _GNU_SOURCE
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <pthread.h>
#include <unistd.h>
#include <string.h>
#include <time.h>

long rdtsc(void)
{
	unsigned int hi, lo;
	__asm__ __volatile__("rdtsc" : "=a"(lo), "=d"(hi));
	return ((long)lo) | (((long)hi) << 32);
}

long get_time_in_nanos(long start_tsc, long end_tsc)
{
	// Our setup is a 1.9 GHz CPU
	return (end_tsc - start_tsc) * 10 / 19;
}

unsigned int simple_get_rand(unsigned int last_rand)
{
	return ((long)last_rand * 1103515245 + 12345) & 0x7fffffff;
}

size_t up_align(size_t size, size_t alignment)
{
	return ((size) + ((alignment)-1)) & ~((alignment)-1);
}

#define PAGE_SIZE 4096 // Typical page size in bytes
// Single thread test, will be executed 4096 times;
// 8 thread test will be executed 512 times;
// 128 thread test will be executed 32 times, etc.
// Statistics are per-thread basis.
#define TOT_THREAD_RUNS 4096

// How many times should the p99 lat exceed the avg lat
#define P99LAT_LIMIT 10.0

char *const BASE_PTR = (char *)0x100000000UL;

#define RESULT_FILE "results"

int DISPATCH_LIGHT;

typedef struct {
	char *base;
	long *offset;
	int thread_id;
	int tot_threads;
	// Wait until interval to proceed the next interrupt
	long interval;
	int is_unfixed_mmap_test;

	// Pass the result back to the main thread
	// Latency (ns)
	long lat;
	// Throughput per sec
	long tput;
} thread_data_t;

typedef struct {
	size_t num_requests_per_thread;
	size_t num_pages_per_request;
	int mmap_before_spawn;
	int trigger_fault_before_spawn;
	int multi_vma_assign_requests;
	int far_assign_requests;
	int rand_assign_requests;
	int is_unfixed_mmap_test;
} test_config_t;

// Throughput per sec
typedef struct {
	long avgtput;
	long avglat;
	long medlat;
	long p99lat;
} throughput_result_t;

// Decls

int entry_point(int argc, char *argv[], void *(*worker_thread)(void *),
		test_config_t config);
void run_test_specify_threads(int num_threads, void *(*worker_thread)(void *),
			      test_config_t config);
void run_test_and_print(int num_threads, void *(*worker_thread)(void *),
			test_config_t config);
void run_test_specify_rounds(int num_threads, void *(*worker_thread)(void *),
			     test_config_t config, long interval,
			     throughput_result_t *result);
void run_test_forked(int num_threads, void *(*worker_thread)(void *),
		     test_config_t config, long interval);
void run_test(int num_threads, void *(*worker_thread)(void *),
	      test_config_t config, long interval);

// Impls

int entry_point(int argc, char *argv[], void *(*worker_thread)(void *),
		test_config_t config)
{
	int num_threads;
	if (argc == 1) {
		num_threads = -1;
	} else if (argc == 2) {
		num_threads = atoi(argv[1]);
	} else {
		fprintf(stderr, "Usage: %s [num_threads]\n", argv[0]);
		exit(EXIT_FAILURE);
	}

	run_test_specify_threads(num_threads, worker_thread, config);

	return 0;
}

void run_test_specify_threads(int num_threads, void *(*worker_thread)(void *),
			      test_config_t config)
{
	printf("Threads, Tot Tput (ops/s), Avg Lat (ns), Med Lat (ns), p99 Lat (ns)\n");

	// Gets the number of CPUS via sched_affinity
	cpu_set_t cpuset;
	if (sched_getaffinity(0, sizeof(cpu_set_t), &cpuset) != 0) {
		perror("sched_getaffinity failed");
		exit(EXIT_FAILURE);
	}
	int num_cpus = CPU_COUNT(&cpuset);

	if (num_threads == -1) {
		int threads[] = { 1, 2, 4, 8, 16, 32, 48, 64, 80, 96, 112, 128 };
		for (int i = 0; i < sizeof(threads) / sizeof(int); i++) {
			if (threads[i] > num_cpus)
				break;
			run_test_and_print(threads[i], worker_thread, config);
		}
	} else {
		run_test_and_print(num_threads, worker_thread, config);
	}
}

void run_test_and_print(int num_threads, void *(*worker_thread)(void *),
			test_config_t config)
{
	throughput_result_t result;
	run_test_specify_rounds(num_threads, worker_thread, config, 0, &result);
	printf("%d, %ld, %ld, %ld, %ld\n", num_threads,
	       result.avgtput * num_threads, result.avglat, result.medlat,
	       result.p99lat);
}

pthread_t threads[TOT_THREAD_RUNS];
thread_data_t thread_data[TOT_THREAD_RUNS];
long thread_tput[TOT_THREAD_RUNS];
long thread_lat[TOT_THREAD_RUNS];

void run_test_specify_rounds(int num_threads, void *(*worker_thread)(void *),
			     test_config_t config, long interval,
			     throughput_result_t *result)
{
	remove(RESULT_FILE);

	int runs = TOT_THREAD_RUNS / num_threads;
	for (int run_id = 0; run_id < runs; run_id++) {
		run_test_forked(num_threads, worker_thread, config, interval);
	}

	// Read throughputs and latencies data from RESULT_FILE
	FILE *file = fopen(RESULT_FILE, "r");
	if (file == NULL) {
		perror("fopen failed");
		exit(EXIT_FAILURE);
	}
	int tot_runs = num_threads * runs;
	long tput = 0, lat = 0;
	for (int i = 0; i < tot_runs; i++) {
		if (fscanf(file, "%ld", &tput) != 1) {
			perror("Incorrect number of runs");
			exit(EXIT_FAILURE);
		}
		if (fscanf(file, "%ld", &lat) != 1) {
			perror("Incorrect number of runs");
			exit(EXIT_FAILURE);
		}
		thread_tput[i] = tput;
		thread_lat[i] = lat;
	}
	if (fscanf(file, "%ld", &tput) == 1) {
		perror("Incorrect number of runs");
		exit(EXIT_FAILURE);
	}
	fclose(file);

	long avgtput = 0, avglat = 0, medlat = 0, p99lat = 0;

	for (int i = 0; i < tot_runs; i++) {
		avgtput += thread_tput[i];
		avglat += thread_lat[i];
	}
	avgtput /= tot_runs;
	avglat /= tot_runs;

	// Calculate the p99 lat
	// bubble sort
	for (int i = 0; i < tot_runs; i++) {
		for (int j = i + 1; j < tot_runs; j++) {
			if (thread_lat[i] > thread_lat[j]) {
				long temp = thread_lat[i];
				thread_lat[i] = thread_lat[j];
				thread_lat[j] = temp;
			}
		}
	}
	medlat = thread_lat[tot_runs / 2];
	p99lat = thread_lat[tot_runs * 99 / 100];

	result->avgtput = avgtput;
	result->avglat = avglat;
	result->medlat = medlat;
	result->p99lat = p99lat;

	return;
}

void run_test_forked(int num_threads, void *(*worker_thread)(void *),
		     test_config_t config, long interval)
{
	// Spawn a process for a test in order to avoid interference between tests
	int pid = fork();
	if (pid == -1) {
		perror("fork failed");
		exit(EXIT_FAILURE);
	} else if (pid == 0) {
		// Child process
		run_test(num_threads, worker_thread, config, interval);
		exit(EXIT_SUCCESS);
	} else {
		// Parent process
		wait(NULL);
	}
}

void run_test(int num_threads, void *(*worker_thread)(void *),
	      test_config_t cfg, long interval)
{
	size_t num_tot_requests = cfg.num_requests_per_thread * num_threads;

	int offset_size = up_align(num_tot_requests * sizeof(long), PAGE_SIZE);
	long *offset = mmap(NULL, offset_size, PROT_READ | PROT_WRITE,
			    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);

	unsigned long reserved_region_size = -1;
	if (cfg.far_assign_requests) {
		reserved_region_size = 512UL * PAGE_SIZE;
	} else {
		reserved_region_size = cfg.num_pages_per_request * PAGE_SIZE;
	}
	for (int i = 0; i < num_tot_requests; i++) {
		offset[i] = i * reserved_region_size;
	}

	char *base = NULL;
	if (cfg.mmap_before_spawn) {
		// munmap or pagefault tests
		if (cfg.multi_vma_assign_requests) {
			// Each request is in one VMA
			base = BASE_PTR;
			for (int i = 0; i < num_tot_requests; i++) {
				char *temp = mmap(
					base + offset[i],
					cfg.num_pages_per_request * PAGE_SIZE,
					PROT_READ | PROT_WRITE,
					MAP_PRIVATE | MAP_FIXED | MAP_ANONYMOUS,
					-1, 0);
				if (temp == MAP_FAILED) {
					perror("mmap failed");
					exit(EXIT_FAILURE);
				}
			}
		} else {
			// All requests are in one VMA
			base = mmap(NULL,
				    num_tot_requests * reserved_region_size,
				    PROT_READ | PROT_WRITE,
				    MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
			if (base == MAP_FAILED) {
				perror("mmap failed");
				exit(EXIT_FAILURE);
			}
		}

		if (cfg.trigger_fault_before_spawn) {
			// Trigger page faults before spawning threads
			for (int i = 0; i < num_tot_requests; i++) {
				char *region = base + offset[i];
				for (int j = 0; j < cfg.num_pages_per_request;
				     j++)
					region[j * PAGE_SIZE] = 1;
			}
		}
	} else {
		// mmap tests
		if (cfg.is_unfixed_mmap_test) {
			base = NULL;
		} else {
			base = BASE_PTR;
		}
	}

	if (cfg.rand_assign_requests) {
		// Random shuffle
		unsigned int rand = 0xdeadbeef - num_threads;
		for (int i = num_tot_requests - 1; i > 0; i--) {
			rand = simple_get_rand(rand);
			int j = rand % (i + 1);
			long temp = offset[i];
			offset[i] = offset[j];
			offset[j] = temp;
		}
	}

	// Initialize global variables
	__atomic_clear(&DISPATCH_LIGHT, __ATOMIC_RELEASE);

	// Create threads and trigger page faults in parallel
	for (int i = 0; i < num_threads; i++) {
		thread_data[i].base = base;
		thread_data[i].offset =
			offset + i * cfg.num_requests_per_thread;
		thread_data[i].thread_id = i;
		thread_data[i].tot_threads = num_threads;
		thread_data[i].interval = interval;
		thread_data[i].is_unfixed_mmap_test = cfg.is_unfixed_mmap_test;

		if (pthread_create(&threads[i], NULL, worker_thread,
				   &thread_data[i]) != 0) {
			perror("pthread_create failed");
			exit(EXIT_FAILURE);
		}

		// Set the thread affinity to a specific core
		cpu_set_t cpuset;
		CPU_ZERO(&cpuset);
		CPU_SET(i, &cpuset);
		if (pthread_setaffinity_np(threads[i], sizeof(cpu_set_t),
					   &cpuset) != 0) {
			perror("pthread_setaffinity_np failed");
			exit(EXIT_FAILURE);
		}
	}

	// Signal all threads to start
	__atomic_store_n(&DISPATCH_LIGHT, 1, __ATOMIC_RELEASE);

	// Join threads
	for (int i = 0; i < num_threads; i++) {
		pthread_join(threads[i], NULL);
	}

	// Write throughputs and latencies data to RESULT_FILE
	FILE *file = fopen(RESULT_FILE, "a");
	if (file == NULL) {
		perror("fopen failed");
		exit(EXIT_FAILURE);
	}
	for (int i = 0; i < num_threads; i++) {
		fprintf(file, "%ld\n%ld\n", thread_data[i].tput,
			thread_data[i].lat);
	}
	fclose(file);
}
