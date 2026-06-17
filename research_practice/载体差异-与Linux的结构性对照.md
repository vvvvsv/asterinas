# 与 Linux 的对照 ＋ 代码层面的抽象优化

> **定位（必读）**：ptrace 对外语义与 ABI 必须对齐 Linux，这部分是「忠实重实现」，不是创新点。
> 本文收两类可讲的东西：
> **A. 载体结构差异**——同一件事，Linux 因单体/非安全/扁平 `task_struct` 这么做，我们因框内核/安全 Rust/分层对象模型那样做（经源码核对）。
> **B. 代码层面的抽象与去重（coding 优化）**——实现里把多处分散逻辑收敛成单一抽象、单一真相源、编译期保证的工程做法。
> 措辞建议：A 用「结构性差异」，B 用「工程优化/可维护性」，都慎用「创新」。

---

# Part A — 与 Linux 的结构性对照

## A1 — 在分层 POSIX 对象模型上重建跟踪关系，而非扁平 `task_struct`

**Linux**：单一 `task_struct` 承载进程/线程/调度实体；ptrace 关系靠内嵌的
`ptraced`/`ptrace_entry` 链表维护，受**全局 `tasklist_lock`** 保护。

**Asterinas**：对象分层为 `Process` / `PosixThread` / `Thread` / `Task`，跟踪关系须
**跨 `Process` ↔ `Thread` 边界**重建：tracer 用 `Weak<Thread>` 持有（防 `Arc` 引用环），
tracee 表为 `BTreeMap<Tid, Arc<Thread>>`，用**细粒度 `Mutex` + 显式锁序**
（`// Lock order: tracer.tracees -> tracee.tracee_status`）取代全局大锁。

> 代码：`kernel/src/process/posix_thread/ptrace/mod.rs`（`attach_to`/`clear_tracees`/`TraceeStatus`）

| | Linux | Asterinas |
|---|---|---|
| 承载实体 | 单一 `task_struct` | `Process`/`PosixThread`/`Thread`/`Task` 分层 |
| 关系存储 | 内嵌链表 | `Weak<Thread>` + `BTreeMap<Tid, Arc<Thread>>` |
| 并发保护 | 全局 `tasklist_lock` | 细粒度 `Mutex` + 显式锁序 |

**可讲的一句**：我们把 Linux 中由全局 `tasklist_lock` 保护的扁平 ptrace 关系表，重建为分层对象模型之上、基于 `Arc`/`Weak` 所有权与细粒度锁的跟踪关系。

## A2 — 全安全 Rust 的调试通路：碰最危险的操作却零裸指针

**Linux**：改寄存器、写别人内存、改控制流——内核里最易出内存安全漏洞的三件事——全经裸指针 + `copy_to_user`/`get_user_pages`。

**Asterinas（框内核：`unsafe` 仅 `ostd`，`kernel/` 全安全 Rust）**：跨进程访问拿物理页**句柄 `UFrame`** 而非裸地址，读写经 fallible 的 `VmReader`/`VmWriter`；寄存器经 `CUserRegsStruct` + 声明式策略表，字段级校验且部分**比 Linux 更严**（注释原文 `"These are more strict than Linux"`）。越界/释放后访问/非法写由类型与所有权在**编译期**约束。

> 代码：`kernel/src/vm/vmar/vmar_impls/access_alien.rs`、`kernel/src/arch/x86/ptrace.rs`

**可讲的一句**：在内核中最易出内存安全漏洞的调试路径上，我们做到零 `unsafe`，危险性由所有权与类型系统在编译期约束，而非靠运行期检查与人工审阅。

## A3 — 寄存器快照 + 持锁，而非原地编辑内核栈 pt_regs（已核 Linux v6.16）

**Linux（已核 v6.16）**：tracer 经 `getreg/putreg` **原地读写** tracee 内核栈上的
`task_pt_regs(target)`，无快照：

```c
// arch/x86/kernel/ptrace.c
getreg():  return *pt_regs_access(task_pt_regs(task), offset);
putreg():  *pt_regs_access(task_pt_regs(child), offset) = value;
```

因改的是活的内核栈，`ptrace_check_attach` 必须先确认 tracee 已切下 CPU：

```c
// kernel/ptrace.c — ptrace_check_attach()
WARN_ON_ONCE(!wait_task_inactive(child, __TASK_TRACED|TASK_FROZEN))
// kernel-doc: "the child is guaranteed to be traced and not executing."
```

且其现场是**碎的**：GP 在内核栈、`fs_base/gs_base` 在 `task->thread`、debug regs 在 `thread->ptrace_bps/...`。

**Asterinas**：进入 ptrace-stop 时、在阻塞**之前**把 `user_ctx.general_regs()` 快照进
持锁的 `TraceeState.general_regs`（一份连续 `CUserRegsStruct`，含 GP+fsbase/gsbase，DR 仿真），
tracer 在快照上读写、resume 回写，一致性由 mutex 保证，**与是否下 CPU 无关**。

> 代码：`kernel/src/process/posix_thread/ptrace/mod.rs`（`do_ptrace_stop` → `get_regs`/`set_regs`/`peek_user`）

| | Linux v6.16（已核） | Asterinas |
|---|---|---|
| GP 现场 | 内核栈 `task_pt_regs()` 原地改 | `TraceeState` 快照，resume 回写 |
| 段基址/DR | 散在 `task->thread` | 并入同一快照 / DR 仿真 |
| 一致性 | `wait_task_inactive` 等下 CPU | 持 mutex，与是否下 CPU 无关 |

**可讲的一句**：经核对 Linux v6.16，tracer 原地编辑 tracee 内核栈的 `task_pt_regs`，故必须用 `wait_task_inactive` 保证其「not executing」；我们改为进入 stop 时把现场快照进持锁的连续 `CUserRegsStruct`，一致性由锁保证、与线程是否在 CPU 上无关。

---

# Part B — 代码层面的抽象与去重（coding 优化）

> 这部分主题：**单一抽象、单一真相源、编译期保证**。
> ⚠️ 诚实标注：只有 **B1** 是「Linux 真的写了好几遍、我们一处」的对照；**B2/B3** 里 Linux 其实**也已收敛**，它们只是「我们的实现同样干净」，**不构成对 Linux 的优势**，讲时不要往「比 Linux 好」上靠。

## B1 — x86 寄存器字段处理：Linux 重列多遍，我们一张表（已核 Linux v6.16）

**这是本节唯一一条「Linux 多处 → 我们一处」的真对照。**

**Linux（已核 v6.16，`arch/x86/kernel/ptrace.c`）**：每个寄存器字段「是不是段寄存器、是不是
eflags、fsbase 怎么校验」这套特殊处理被**重列多遍**——

- `getreg()`（读，64 位）、`putreg()`（写，64 位）各自独立分派一遍；
- 32 位 compat 的 `getreg32()` / `putreg32()` 用 `SEG32()` 宏**把段寄存器、eflags 又重列一遍**，不复用 64 位分派；
- 段寄存器核心逻辑还按 `CONFIG_X86_32/64` 分裂成两份 `set/get_segment_reg`。

即「哪个字段要特殊处理」散在约 **4 个分派函数**（读侧/写侧 × 64/32 位）。

**Asterinas**：单张 `REG_RULES` 表为每个字段声明**一次**策略
（`Policy::{Set, SetIf, SetBitsTruncate, Fixed}`），然后**四种访问共查同一张表**：
读快照 `From<&GeneralRegs>`、写校验 `apply_to`、按偏移读 `read_user_word`(PEEKUSER)、
按偏移写 `write_user_word`(POKEUSER)。「rax 自由写 / cs 是 Fixed / rflags 只放可控位 / rsp 必须用户地址」只写一遍。

外加两道**编译期保证**：

- `const _: () = assert!((REG_RULES.len()+1)*size_of::<usize>() == size_of::<CUserRegsStruct>());`
  ——漏一条规则或改了结构体，**编译就失败**；
- `const_assert!(LINUX_USER_CS == USER_CS_VALUE)` 守住段选择子取值。

> 代码：`kernel/src/arch/x86/ptrace.rs`（`REG_RULES`/`RegRule`/`Policy`/`From`/`apply_to`/`read_user_word`/`write_user_word`）

| | Linux v6.16（已核） | Asterinas |
|---|---|---|
| 字段特殊处理声明在哪 | `getreg`/`putreg`（+compat `getreg32`/`putreg32`）约 4 处分派 | 单张 `REG_RULES` 表，声明一次 |
| 读 vs 写 | 读侧写侧各列一遍 | 同一张表双向驱动 |
| 整体 vs 按偏移 | regset 路径循环调用 getreg/putreg | 同一张表，四入口共查 |
| 漏写/漂移防护 | 靠 review | 编译期 size 断言 + `const_assert` |

> **量化（诚实口径）**：不含 32 位 compat 是 **2:1**（读 `getreg` + 写 `putreg` → 一张表），含 compat 约 **4:1**。
> ⚠️ 边界：Linux 重复有一大半来自 **32 位 compat 路径**，我们部分是因为**不跑 32 位用户代码**才省掉的——讲 4:1 时要带这句，否则别报 4:1，老实说 2:1。

**可讲的一句**
> x86 寄存器的字段级特殊处理，Linux 在 `getreg`/`putreg` 读写两侧各列一遍、32 位 compat 又重列一遍；我们用单张 `REG_RULES` 策略表声明一次，读快照、写校验、按偏移 PEEK/POKEUSER 四条路径共查同表，并以编译期断言防止表与结构体漂移。

## B2 — 三类 ptrace-stop 收敛到单一 `do_ptrace_stop()`

信号停（`ptrace_stop`）、syscall 停（`ptrace_may_stop_on_syscall`）、事件停
（`ptrace_may_stop_on`）三条来源各自构造 `signal` 与 `wait_status` 后，**全部汇入同一个**
`do_ptrace_stop()`：统一保存现场、记录 wait status、投 `SIGCHLD`、`Waiter` 阻塞、resume 回写。

> 代码：`kernel/src/process/posix_thread/ptrace/mod.rs`（`do_ptrace_stop`）
> **价值**：停止/恢复的核心时序只有一处实现，三种停止只负责「准备参数」，新增停止类型零成本接入。
> ⚠️ **不构成对 Linux 的优势**：Linux 同样收敛——`ptrace_event`→`ptrace_notify`→`ptrace_stop`、syscall 经 `ptrace_report_syscall`→`ptrace_notify`→`ptrace_stop`、signal-stop 直接进 `ptrace_stop`，三类停止在 Linux 也都汇到 `ptrace_stop()`。所以这条只能讲「我们的实现同样干净」，**不能讲「Linux 分散、我们合一」**。

## B3 — 跨进程读写收敛到单一 `access_alien` 原语（一套循环、闭包区分操作）

`read_alien` / `write_alien` / `fill_zeros_alien` 共用同一个泛型
`access_alien<F: FnMut(UFrame, usize)>`：分页对齐、按页定位物理帧、缺页则强制触发
page fault 重试、跨页拼接——这套**易错的循环只写一遍**，三种操作只是传入不同闭包 `op`。

> 代码：`kernel/src/vm/vmar/vmar_impls/access_alien.rs`（`access_alien`/`read_alien`/`write_alien`/`fill_zeros_alien`）
> **价值**：跨进程、跨页、带缺页重试的读写是 bug 高发区，收敛成「一套循环 + 闭包」后，读/写/清零行为天然一致。
> ⚠️ **不构成对 Linux 的优势**：「ptrace PEEK/POKE 与 `/proc/pid/mem` 共用一套底层」在 Linux 里**也是如此**——两者都走 `access_remote_vm()`。所以别讲「Linux 各写一遍、我们共用」；这条只能讲「读/写/清零在我们这里共用一套循环 + 闭包」这一层 our-side 去重。

---

# 答辩问答防线

- **「这算创新还是移植？」** → 「高质量移植 + 借安全语言载体做的结构性加固 + 实现层的抽象收敛」。对外 ABI 必须对齐 Linux，工作在内部结构与代码质量。
- **「Part A 哪条最硬？」** → A3：有 Linux v6.16 源码逐行背书。
- **「Part B 怎么定位？」** → 工程质量/可维护性。**只有 B1 是「Linux 多遍、我们一处」的真对照（已核 v6.16，2:1～4:1）**；B2/B3 是「实现同样干净」，不构成对 Linux 的优势，别讲成比 Linux 好。
- **底线**：A1/A2 的 delta 源于 Asterinas 这个**载体**，说「我们把 ptrace 适配/重建到了这个载体上」，别说成「我们发明了对象模型」。

---

## 附：已核 Linux 源码出处（v6.16，供 A3 引用）

| 主张 | 文件 / 符号 |
|---|---|
| GP 寄存器原地改 | `arch/x86/kernel/ptrace.c`：`getreg`/`putreg`/`genregs_get`/`genregs_set`/`pt_regs_access`/`task_pt_regs` |
| fsbase/gsbase 在 thread | `arch/x86/kernel/ptrace.c`：`x86_fsbase_read_task`/`x86_gsbase_read_task` |
| debug regs 在 thread | `arch/x86/kernel/ptrace.c`：`ptrace_get_debugreg`/`ptrace_set_debugreg` |
| 必须等 tracee 下 CPU | `kernel/ptrace.c`：`ptrace_check_attach` → `wait_task_inactive(child, __TASK_TRACED|TASK_FROZEN)` |
