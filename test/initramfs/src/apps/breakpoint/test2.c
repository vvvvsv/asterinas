#include <stdio.h>
#include <unistd.h>

void hello_world(int x) {
    printf("Hello, World %d!\n", x);
}

int main(void) {
    for (int i = 0; i < 5; i++) {
        hello_world(i);
        sleep(1);
    }
    return 0;
}