#include "common.h"

#define NUM_MMAPS 512 // Number mmaps per thread
#define PAGES_PER_MMAP 8

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

	if (data->is_unfixed_mmap_test) {
		// map them one by one
		for (size_t i = 0; i < NUM_MMAPS; i++) {
			long req_start = rdtsc();

			mmap(NULL, PAGE_SIZE * PAGES_PER_MMAP,
			     PROT_READ | PROT_WRITE,
			     MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);

			long req_end = rdtsc();
			tot_lat += req_end - req_start;
			while (req_end - req_start <= data->interval) {
				req_end = rdtsc();
			}
		}
	} else {
		// map them one by one
		for (size_t i = 0; i < NUM_MMAPS; i++) {
			long req_start = rdtsc();

			mmap(data->base + data->offset[i],
			     PAGE_SIZE * PAGES_PER_MMAP, PROT_READ | PROT_WRITE,
			     MAP_PRIVATE | MAP_FIXED | MAP_ANONYMOUS, -1, 0);

			long req_end = rdtsc();
			tot_lat += req_end - req_start;
			while (req_end - req_start <= data->interval) {
				req_end = rdtsc();
			}
		}
	}

	tsc_end = rdtsc();

	long tot_time = get_time_in_nanos(tsc_start, tsc_end);

	data->tput = 1000000000L * NUM_MMAPS / tot_time;
	data->lat = get_time_in_nanos(0, tot_lat) / NUM_MMAPS;

	return NULL;
}

int main(int argc, char *argv[])
{
	int ret;

	printf("***MMAP UNFIXED***\n");
	ret = entry_point(argc, argv, worker_thread,
			  (test_config_t){ .num_requests_per_thread = NUM_MMAPS,
					   .num_pages_per_request =
						   PAGES_PER_MMAP,
					   .mmap_before_spawn = 0,
					   .trigger_fault_before_spawn = 0,
					   .multi_vma_assign_requests = 0,
					   .far_assign_requests = 0,
					   .rand_assign_requests = 0,
					   .is_unfixed_mmap_test = 1 });
	if (ret != 0) {
		perror("entry_point failed");
		exit(EXIT_FAILURE);
	}
	printf("\n");

	for (int far_near = 0; far_near <= 1; far_near++) {
		for (int dist_rand = 0; dist_rand <= 1; dist_rand++) {
			printf("***MMAP FIXED %s %s***\n",
			       far_near ? "FAR" : "NEAR",
			       dist_rand ? "RAND" : "DIST");
			ret = entry_point(
				argc, argv, worker_thread,
				(test_config_t){
					.num_requests_per_thread = NUM_MMAPS,
					.num_pages_per_request = PAGES_PER_MMAP,
					.mmap_before_spawn = 0,
					.trigger_fault_before_spawn = 0,
					.multi_vma_assign_requests = 0,
					.far_assign_requests = far_near,
					.rand_assign_requests = dist_rand,
					.is_unfixed_mmap_test = 0 });
			if (ret != 0) {
				perror("entry_point failed");
				exit(EXIT_FAILURE);
			}
			printf("\n");
		}
	}
}