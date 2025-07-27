#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h> // read, write, close
#include <arpa/inet.h> // inet_pton
#include <netdb.h> // getaddrinfo
#include <sys/socket.h>

#define BUFFER_SIZE 4096

int main()
{
	const char *host = "127.0.0.1";
	const char *port = "8200";
	const char *path = "/v1/sys/init";

	// 1. 解析地址
	struct addrinfo hints, *res;
	memset(&hints, 0, sizeof hints);
	hints.ai_family = AF_UNSPEC; // IPv4 or IPv6
	hints.ai_socktype = SOCK_STREAM;

	if (getaddrinfo(host, port, &hints, &res) != 0) {
		perror("getaddrinfo");
		return 1;
	}

	// 2. 创建 socket 并连接
	int sockfd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
	if (sockfd < 0) {
		perror("socket");
		freeaddrinfo(res);
		return 1;
	}

	if (connect(sockfd, res->ai_addr, res->ai_addrlen) != 0) {
		perror("connect");
		close(sockfd);
		freeaddrinfo(res);
		return 1;
	}
	freeaddrinfo(res);

	// 3. 构造 HTTP GET 请求
	char request[BUFFER_SIZE];

	int request_len = snprintf(request, sizeof(request),
				   "GET %s HTTP/1.1\r\n"
				   "Host: %s:%s\r\n"
				   "Connection: close\r\n"
				   "\r\n",
				   path, host, port);

	if (request_len >= sizeof(request)) {
		fprintf(stderr, "Request too large\n");
		close(sockfd);
		return 1;
	}

	// 4. 发送请求
	ssize_t sent = 0;
	while (sent < request_len) {
		ssize_t n = write(sockfd, request + sent, request_len - sent);
		if (n <= 0) {
			perror("write");
			close(sockfd);
			return 1;
		}
		sent += n;
	}

	// 5. 读取响应并打印
	char buffer[BUFFER_SIZE];
	ssize_t received;
	while ((received = read(sockfd, buffer, sizeof(buffer) - 1)) > 0) {
		buffer[received] = '\0'; // 以字符串形式打印
		printf("%s\n", buffer);
	}

	if (received < 0) {
		perror("read");
	}

	close(sockfd);
	return 0;
}
