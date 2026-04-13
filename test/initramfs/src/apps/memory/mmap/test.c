#include <stdio.h>
#include <unistd.h>

int main()
{
	printf("Hello before sleeping for 1 second!\n");
	sleep(1); // 睡眠1秒
	printf("Hello after sleeping for 1 second!\n");
	return 0;
}