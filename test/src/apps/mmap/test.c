#include <stdio.h>
#include <unistd.h>
#include <fcntl.h>
#include <string.h>
#include <errno.h>
#include <stdlib.h>

#define SEEK_POS 2

int main()
{
	char buf[256];
	ssize_t n;
	int fd;

	// 1. 打开 /proc/self/oom_score_adj 读取当前值
	fd = open("/proc/self/oom_score_adj", O_RDONLY);
	if (fd < 0) {
		perror("open for read");
		return 1;
	}

	n = read(fd, buf, sizeof(buf) - 1);
	if (n < 0) {
		perror("read");
		close(fd);
		return 1;
	}
	buf[n] = '\0';
	printf("Original oom_score_adj: %s", buf);
	close(fd);

	// 2. 打开 /proc/self/oom_score_adj 写入新值
	fd = open("/proc/self/oom_score_adj", O_WRONLY | O_CREAT | O_TRUNC,
		  0666);
	if (fd < 0) {
		perror("open for write");
		return 1;
	}

	// seek 到文件开头
	if (lseek(fd, SEEK_POS, SEEK_SET) < 0) {
		perror("lseek");
		close(fd);
		return 1;
	}

	const char *new_value = "100\n"; // 新的 oom_score_adj
	n = write(fd, new_value, strlen(new_value) + 1);
	if (n < 0) {
		perror("write");
		close(fd);
		return 1;
	}
	printf("%ld\n", n);
	close(fd);

	// 3. 再次打开 /proc/self/oom_score_adj 读取新值
	fd = open("/proc/self/oom_score_adj", O_RDONLY);
	if (fd < 0) {
		perror("open for read");
		return 1;
	}

	// seek 到文件开头
	if (lseek(fd, 0, SEEK_SET) < 0) {
		perror("lseek");
		close(fd);
		return 1;
	}

	n = read(fd, buf, sizeof(buf) - 1);
	if (n < 0) {
		perror("read");
		close(fd);
		return 1;
	}
	buf[n] = '\0';
	printf("New oom_score_adj: %s", buf);
	close(fd);

	return 0;
}
