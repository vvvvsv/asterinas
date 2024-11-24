#include "common.h"

#define NUM_PAGES 16384 // Number of pages to allocate totally for mmap

void *worker_thread(void *arg)
{
	thread_data_t *data = (thread_data_t *)arg;

	long tsc_start, tsc_end;
	long tot_lat = 0;

	// Wait for the main thread to signal that all threads are ready
	while (__atomic_load_n(&DISPATCH_LIGHT, __ATOMIC_ACQUIRE) == 0) {
		sched_yield();
	}
	tsc_start = rdtsc();

	// Trigger page fault one by one
	for (size_t i = 0; i < NUM_PAGES; i++) {
		long req_start = rdtsc();

		data->base[data->offset[i]] = 1;

		long req_end = rdtsc();
		tot_lat += req_end - req_start;
		while (req_end - req_start <= data->interval) {
			req_end = rdtsc();
		}
	}

	tsc_end = rdtsc();
	long tot_time = get_time_in_nanos(tsc_start, tsc_end);

	data->tput = 1000000000L * NUM_PAGES / tot_time;
	data->lat = get_time_in_nanos(0, tot_lat) / NUM_PAGES;

	return NULL;
}

int main(int argc, char *argv[])
{
	int ret;

	int one_multi = 1;
	int near_far = 0;
	int dist_rand = 0;

	printf("***MEM_USAGE_SIM %s %s %s***\n",
	       one_multi ? "MULTI_VMAS" : "ONE_VMA", near_far ? "FAR" : "NEAR",
	       dist_rand ? "RAND" : "DIST");
	ret = entry_point(argc, argv, worker_thread,
			  (test_config_t){ .num_requests_per_thread = NUM_PAGES,
					   .num_pages_per_request = 1,
					   .mmap_before_spawn = 1,
					   .trigger_fault_before_spawn = 0,
					   .multi_vma_assign_requests =
						   one_multi,
					   .far_assign_requests = near_far,
					   .rand_assign_requests = dist_rand,
					   .is_unfixed_mmap_test = 0 });
	if (ret != 0) {
		perror("entry_point failed");
		exit(EXIT_FAILURE);
	}
	printf("\n");
}
