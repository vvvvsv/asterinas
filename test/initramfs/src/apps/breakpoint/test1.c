#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include <string.h>
#include <sys/wait.h>
#include <elf.h>
#include <errno.h>

#define TARGET "/test/breakpoint/test2"
#define FUNC_NAME "hello_world"
#define READ_SIZE 32

/* ----------------------------- */
static void die(const char *msg)
{
    perror(msg);
    exit(1);
}

/* ----------------------------- */
/* read exactly size bytes */
static void read_full(int fd, void *buf, size_t size)
{
    size_t off = 0;
    while (off < size) {
        ssize_t n = read(fd, (char *)buf + off, size - off);
        if (n < 0)
            die("read");
        if (n == 0) {
            fprintf(stderr, "unexpected EOF\n");
            exit(1);
        }
        off += n;
    }
}

/* ----------------------------- */
/* /proc/pid/maps -> text base */
unsigned long find_text_base(pid_t pid)
{
    char path[64];
    snprintf(path, sizeof(path), "/proc/%d/maps", pid);

    FILE *fp = fopen(path, "r");
    if (!fp)
        die("fopen maps");

    char line[512];
    while (fgets(line, sizeof(line), fp)) {
        if (strstr(line, TARGET) && strstr(line, "r-xp")) {
            unsigned long start;
            sscanf(line, "%lx-", &start);
            fclose(fp);
            printf("%s", line);
            return start;
        }
    }

    fclose(fp);
    fprintf(stderr, "text base not found\n");
    exit(1);
}

/* ----------------------------- */
struct sym_info {
    unsigned long value;   /* st_value */
    int is_pie;
};

/* ----------------------------- */
/* parse ELF symbol */
struct sym_info find_symbol_offset(const char *file,
                                   const char *name)
{
    struct sym_info info = {0};

    int fd = open(file, O_RDONLY);
    if (fd < 0)
        die("open elf");

    Elf64_Ehdr eh;
    read_full(fd, &eh, sizeof(eh));

    if (memcmp(eh.e_ident, ELFMAG, SELFMAG) != 0) {
        fprintf(stderr, "not elf\n");
        exit(1);
    }

    info.is_pie = (eh.e_type == ET_DYN);

    lseek(fd, eh.e_shoff, SEEK_SET);

    Elf64_Shdr sh;
    Elf64_Shdr symtab = {0}, strtab = {0};

    for (int i = 0; i < eh.e_shnum; i++) {
        read_full(fd, &sh, sizeof(sh));

        if (sh.sh_type == SHT_SYMTAB)
            symtab = sh;

        if (sh.sh_type == SHT_STRTAB && i != eh.e_shstrndx)
            strtab = sh;
    }

    if (!symtab.sh_offset || !strtab.sh_offset) {
        fprintf(stderr, "symtab/strtab not found\n");
        exit(1);
    }

    char *strs = malloc(strtab.sh_size);
    if (!strs)
        die("malloc");

    lseek(fd, strtab.sh_offset, SEEK_SET);
    read_full(fd, strs, strtab.sh_size);

    lseek(fd, symtab.sh_offset, SEEK_SET);

    Elf64_Sym sym;
    int n = symtab.sh_size / sizeof(sym);

    for (int i = 0; i < n; i++) {
        read_full(fd, &sym, sizeof(sym));
        if (strcmp(strs + sym.st_name, name) == 0) {
            info.value = sym.st_value;
            free(strs);
            close(fd);
            return info;
        }
    }

    fprintf(stderr, "symbol not found\n");
    exit(1);
}

/* ----------------------------- */
/* VA -> file offset using PT_LOAD */
unsigned long vaddr_to_offset(const char *file,
                              unsigned long vaddr)
{
    int fd = open(file, O_RDONLY);
    if (fd < 0)
        die("open elf");

    Elf64_Ehdr eh;
    read_full(fd, &eh, sizeof(eh));

    lseek(fd, eh.e_phoff, SEEK_SET);

    Elf64_Phdr ph;

    for (int i = 0; i < eh.e_phnum; i++) {
        read_full(fd, &ph, sizeof(ph));

        if (ph.p_type != PT_LOAD)
            continue;

        if (vaddr >= ph.p_vaddr &&
            vaddr < ph.p_vaddr + ph.p_memsz) {

            unsigned long off =
                ph.p_offset + (vaddr - ph.p_vaddr);

            close(fd);
            return off;
        }
    }

    fprintf(stderr, "vaddr not in PT_LOAD\n");
    exit(1);
}

/* ----------------------------- */
/* read child memory */
void read_child_mem(pid_t pid,
                    unsigned long addr,
                    unsigned char *buf)
{
    char path[64];
    snprintf(path, sizeof(path), "/proc/%d/mem", pid);

    int fd = open(path, O_RDONLY);
    if (fd < 0)
        die("open mem");

    size_t off = 0;
    while (off < READ_SIZE) {
        ssize_t n = pread(fd, buf + off,
                          READ_SIZE - off,
                          addr + off);
        if (n <= 0)
            die("pread");
        off += n;
    }

    close(fd);
}

/* ----------------------------- */
/* read ELF file bytes */
void read_file_bytes(const char *file,
                     unsigned long offset,
                     unsigned char *buf)
{
    int fd = open(file, O_RDONLY);
    if (fd < 0)
        die("open file");

    if (lseek(fd, offset, SEEK_SET) < 0)
        die("lseek");

    read_full(fd, buf, READ_SIZE);
    close(fd);
}

/* ----------------------------- */
int main(void)
{
    pid_t pid = fork();
    if (pid == 0) {
        execl(TARGET, TARGET, NULL);
        die("exec");
    }

    sleep(1);

    unsigned long base = find_text_base(pid);
    struct sym_info s = find_symbol_offset(TARGET, FUNC_NAME);

    unsigned long runtime_addr;
    if (s.is_pie)
        runtime_addr = base + s.value;
    else
        runtime_addr = s.value;

    unsigned long file_off =
        vaddr_to_offset(TARGET, s.value);

    printf("[+] ELF type    : %s\n",
           s.is_pie ? "PIE" : "NON-PIE");
    printf("[+] text base   : 0x%lx\n", base);
    printf("[+] sym value   : 0x%lx\n", s.value);
    printf("[+] runtime addr: 0x%lx\n", runtime_addr);
    printf("[+] file offset : 0x%lx\n", file_off);

    unsigned char mem1[READ_SIZE];
    unsigned char mem2[READ_SIZE];
    memset(mem1, 0, sizeof(mem1));
    memset(mem2, 0, sizeof(mem2));

    read_child_mem(pid, runtime_addr, mem1);
    read_file_bytes(TARGET, file_off, mem2);

    printf("\nchild mem:\n");
    for (int i = 0; i < READ_SIZE; i++)
        printf("%02x ", mem1[i]);

    printf("\n\nfile bytes:\n");
    for (int i = 0; i < READ_SIZE; i++)
        printf("%02x ", mem2[i]);

    printf("\n");

    if (memcmp(mem1, mem2, READ_SIZE) == 0)
        printf("\nMATCH ✔\n");
    else
        printf("\nDIFFER ✘\n");

    wait(NULL);
    return 0;
}