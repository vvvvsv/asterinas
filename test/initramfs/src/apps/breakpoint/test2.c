#include <stdio.h>
#include <unistd.h>

void hello_world() {
    printf("Hello, World!\n");
}

int main(void) {
    sleep(5);
    hello_world();
    return 0;
}