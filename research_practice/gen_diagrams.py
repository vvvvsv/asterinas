#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate polished framework diagrams (.dot -> .png) for the defense deck."""
import os, subprocess, textwrap

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "assets")
os.makedirs(OUT, exist_ok=True)

FONT = "PingFang SC"

# ---- shared theme ----
HEADER = f'''
  graph [fontname="{FONT}", bgcolor="white", pad="0.35", {{extra}}];
  node  [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.16,0.10", fontsize=13];
  edge  [fontname="{FONT}", color="#5b6b7d", penwidth=1.4, arrowsize=0.85, fontsize=11];
'''

# palette (fill, border, font)
PAL = {
    "user":   ("#FFF3DC", "#C8881A", "#7a5200"),
    "sys":    ("#E7F0FF", "#2F5FA8", "#1d3a66"),
    "sec":    ("#F1E9FF", "#6F42C1", "#48227f"),
    "core":   ("#E5F8EE", "#2F9D57", "#1c6035"),
    "event":  ("#F8F6E0", "#9A9620", "#5f5c10"),
    "mem":    ("#FFEAEA", "#C0463F", "#7d2723"),
    "proc":   ("#E3F4F4", "#1F8585", "#0f5151"),
    "ink":    ("#EEF1F5", "#3a4a5c", "#22303f"),
}

def node(nid, label, kind="ink", **kw):
    fill, border, font = PAL[kind]
    attrs = f'label="{label}", fillcolor="{fill}", color="{border}", fontcolor="{font}"'
    for k,v in kw.items():
        attrs += f', {k}="{v}"'
    return f'  {nid} [{attrs}];\n'

def cluster(cid, label, kind, body, style="rounded,filled"):
    fill, border, font = PAL[kind]
    # lighten cluster bg
    return (f'  subgraph cluster_{cid} {{\n'
            f'    label="{label}"; fontname="{FONT}"; fontsize=14; fontcolor="{font}";\n'
            f'    style="{style}"; color="{border}"; fillcolor="{fill}99"; penwidth=1.8; margin=14;\n'
            f'{body}  }}\n')

def render(name, dot, engine="dot"):
    path = os.path.join(OUT, name + ".dot")
    with open(path, "w") as f:
        f.write(dot)
    subprocess.run(["/opt/homebrew/bin/dot", f"-K{engine}", "-Tpng", "-Gdpi=150", path,
                    "-o", os.path.join(OUT, name + ".png")], check=True)
    print("rendered", name)

def wrap(dotbody, extra='rankdir=TB, nodesep="0.5", ranksep="0.6"'):
    return "digraph G {\n" + HEADER.replace("{extra}", extra) + dotbody + "}\n"

# =====================================================================
# 1. 总体架构：七层调试能力通路（精美分层框架）
# =====================================================================
b  = node("u1", "GDB 调试器", "user", shape="box")
b += node("u2", "strace 跟踪器", "user")
b += cluster("user", "① 用户态工具链", "user", "    u1; u2;\n")

b += node("s1", "sys_ptrace\\n系统调用分发", "sys")
b += node("s2", "/proc 文件接口\\nmaps · mem · auxv", "sys")
b += cluster("sys", "② Linux 兼容系统调用接口", "sys", "    s1; s2;\n")

b += node("sec1", "ptrace_may_access\\nUID/GID · CAP_SYS_PTRACE", "sec")
b += node("sec2", "Yama LSM\\nptrace_scope 策略", "sec")
b += node("sec3", "alien access 凭证\\nRead/Attach · Fs/Real", "sec")
b += cluster("sec", "③ 安全边界（统一鉴权）", "sec", "    sec1; sec2; sec3;\n")

b += node("c1", "tracer / tracee 关系\\nArc + Weak · 固定锁序", "core")
b += node("c2", "ptrace-stop 主状态机\\n保存现场 · 阻塞 · 恢复", "core")
b += node("c3", "resume 语义\\nCONT · STEP · SYSCALL", "core")
b += cluster("core", "④ ptrace 核心状态机", "core", "    c1; c2; c3;\n")

b += node("e1", "signal", "event")
b += node("e2", "syscall", "event")
b += node("e3", "exec / exit", "event")
b += node("e4", "clone family", "event")
b += cluster("event", "⑤ 协作事件源（统一触发停止）", "event", "    e1; e2; e3; e4;\n")

b += node("m1", "VMAR alien access\\n跨进程内存读写", "mem")
b += node("m2", "寄存器快照\\nCUserRegsStruct · USER area", "mem")
b += cluster("res", "⑥ 资源访问原语", "mem", "    m1; m2;\n")

b += node("w1", "wait / waitpid 报告\\nTraceeStop · TraceeExit → wait4", "proc")

b += '''
  u1 -> s1 [label="ptrace(2)"];
  u2 -> s1 [label="ptrace(2)"];
  u1 -> s2 [label="open/read"];  u2 -> s2;
  s1 -> sec1; s2 -> sec1 [style=dashed];
  sec1 -> c1; sec2 -> c1 [style=dashed]; sec3 -> m1 [style=dashed];
  c1 -> c2; c2 -> c3 [dir=both];
  e1 -> c2; e2 -> c2; e3 -> c2; e4 -> c2 [style=dashed, label="暂拒绝"];
  c2 -> m1; c2 -> m2; s2 -> m1 [label="复用"];
  c2 -> w1 [label="SIGCHLD"]; w1 -> u1 [style=dashed, constraint=false]; w1 -> u2 [style=dashed, constraint=false];
'''
render("01_arch_overview", wrap(b, extra='rankdir=TB, nodesep="0.45", ranksep="0.55", compound=true'))

# =====================================================================
# 2. ptrace 请求分发：入口薄、状态机厚
# =====================================================================
b  = node("g", "GDB / strace\\nptrace(request, tid, addr, data)", "user")
b += node("p", "sys_ptrace 解析 PtraceRequest", "sys")
b += node("attach", "TRACEME\\n建立 tracer/tracee 关系\\n+ 权限检查", "sec")
b += node("look", "get_tracee(tid)\\n校验追踪关系", "core")
b += node("mem", "PEEK/POKE TEXT·DATA\\n读写 tracee 内存", "mem")
b += node("ua",  "PEEK/POKE USER · GET/SETREGS\\n寄存器与 USER area", "mem")
b += node("cont","CONT · SINGLESTEP · SYSCALL\\n注入信号 + 恢复运行", "core")
b += node("opt", "SETOPTIONS · GETEVENTMSG · GETSIGINFO\\n事件与 siginfo", "event")
b += '''
  g -> p; p -> attach; p -> look;
  look -> mem; look -> ua; look -> cont; look -> opt;
'''
render("02_ptrace_dispatch", wrap(b, extra='rankdir=LR, nodesep="0.4", ranksep="0.7"'))

# =====================================================================
# 3. tracer / tracee 关系与固定锁序
# =====================================================================
b  = node("tracer", "Tracer 线程\\n(GDB 主线程)", "user")
b += node("map", "tracees: BTreeMap&lt;Tid, Arc&lt;Thread&gt;&gt;\\n强引用持有 tracee", "core")
b += node("tracee", "Tracee 线程\\n(被调试进程)", "proc")
b += node("status", "TraceeStatus\\nis_stopped · state", "core")
b += node("back", "TraceeState.tracer: Weak&lt;Thread&gt;\\n反向弱引用（打破环）", "mem")
b += '''
  tracer -> map [label="持有"];
  map -> tracee [label="Arc 强引用", color="#2F9D57", penwidth=2.2];
  tracee -> status [label="拥有"];
  status -> back;
  back -> tracer [label="Weak 弱引用", style=dashed, color="#C0463F", penwidth=2.2, constraint=false];
  { rank=same; map; status; }
'''
b += '''
  note [shape=note, fontname="''' + FONT + '''", fontsize=12, fillcolor="#FFF8E6", color="#C8881A", style="filled",
        label="固定锁序：tracer.tracees  →  tracee.tracee_status\\nwait / resume / detach / exit 清理均按同一方向加锁\\n避免遍历与停止/退出之间交叉等待死锁"];
'''
render("03_tracer_tracee", wrap(b, extra='rankdir=LR, nodesep="0.6", ranksep="0.9"'))

# =====================================================================
# 4. ptrace-stop 主状态机（生命周期）
# =====================================================================
b  = node("untraced", "未被跟踪", "ink", shape="ellipse")
b += node("attached", "已建立 trace 关系", "user", shape="ellipse")
b += node("stopped", "ptrace-stop\\n保存寄存器现场\\n记录信号/事件 + wait status\\nis_stopped = true", "core", shape="ellipse")
b += node("reported", "wait 报告\\nSIGCHLD(CLD_TRAPPED)", "proc", shape="ellipse")
b += node("running", "恢复运行\\n回写寄存器快照\\nis_stopped = false", "user", shape="ellipse")
b += node("exited", "退出 / 清理\\nTraceeExit + detach", "mem", shape="ellipse")
b += '''
  untraced -> attached [label="TRACEME / attach"];
  attached -> stopped [label="signal / syscall / exec / exit"];
  stopped -> reported [label="SIGCHLD + 唤醒 wait"];
  reported -> stopped [label="WNOWAIT / 尚未 continue", style=dashed, constraint=false];
  stopped -> running [label="CONT / SINGLESTEP / SYSCALL"];
  running -> stopped [label="下一次停止条件", constraint=false];
  stopped -> exited [label="SIGKILL 打断 / exit"];
  running -> exited [label="exit"];
'''
render("04_state_machine", wrap(b, extra='rankdir=LR, nodesep="0.5", ranksep="0.7"'))

# =====================================================================
# 5. 三类停止统一收敛到 do_ptrace_stop
# =====================================================================
b  = node("sig", "信号投递停\\nsignal-delivery-stop\\n出队非 SIGKILL 信号", "event")
b += node("sys", "系统调用停\\nsyscall entry / exit\\nTRACESYSGOOD 编码", "event")
b += node("evt", "事件停\\nexec / exit event-stop\\n受 PtraceOptions 控制", "event")
b += node("hub", "do_ptrace_stop()\\n① 保存寄存器现场\\n② 记录信号/事件 + wait status\\n③ 投递 SIGCHLD 通知 tracer\\n④ 阻塞 tracee（StopByPtrace）\\n⑤ resume 后回写并返回", "core",
          shape="box", penwidth="2.4", fontsize="14")
b += node("wait", "tracer 看到 Linux 风格\\nwait4 状态字", "proc")
b += '''
  sig -> hub; sys -> hub; evt -> hub;
  hub -> wait [label="统一出口"];
  { rank=same; sig; sys; evt; }
'''
b += '''
  note [shape=note, fontsize=12, fontname="''' + FONT + '''", fillcolor="#E5F8EE", color="#2F9D57", style="filled",
        label="设计取舍：ptrace-stop 不另造调度语义，\\n复用既有 signal 与 wait 路径"];
'''
render("05_three_stops", wrap(b, extra='rankdir=TB, nodesep="0.45", ranksep="0.7"'))

# =====================================================================
# 6. StopDeliverySignal 四态机 + resume
# =====================================================================
b  = node("empty", "Empty\\n无停止信号", "ink", shape="ellipse")
b += node("pending", "Pending\\n等待 wait 观察", "event", shape="ellipse")
b += node("consumed", "Consumed\\nwait 已报告", "proc", shape="ellipse")
b += node("injected", "Injected\\ntracer 注入新信号", "mem", shape="ellipse")
b += '''
  empty -> pending [label="ptrace-stop: stop()"];
  pending -> consumed [label="wait（非 WNOWAIT）"];
  pending -> pending [label="wait + WNOWAIT", constraint=false];
  pending -> injected [label="resume 注入/替换"];
  consumed -> injected [label="resume 注入/替换"];
  pending -> empty [label="resume 抑制 / 清理"];
  consumed -> empty [label="resume 抑制"];
  injected -> empty [label="信号投递后清空"];
'''
b += '''
  note [shape=note, fontsize=12, fontname="''' + FONT + '''", fillcolor="#FFF8E6", color="#C8881A", style="filled",
        label="四态保证：同一停止不被 wait 重复报告；\\nwait 返回后信号不丢失；\\ntracer 仍可在 CONT 时替换 / 注入 / 抑制"];
'''
render("06_signal_states", wrap(b, extra='rankdir=LR, nodesep="0.55", ranksep="0.8"'))

# =====================================================================
# 7. 跨进程内存访问：VMAR alien access（不切换页表）
# =====================================================================
b  = node("src1", "ptrace PEEK / POKE", "user")
b += node("src2", "/proc/&lt;pid&gt;/mem", "user")
b += node("chk", "权限 + 停止态检查", "sec")
b += node("entry", "read_alien / write_alien / fill_zeros_alien\\n→ access_alien() 统一原语", "core")
b += node("query", "query_page_with_required_flags\\n在目标 VmSpace 按页查询映射/权限\\n（不切换地址空间）", "core")
b += node("frame", "命中 RAM UFrame\\nframe.reader()/writer() 直接拷贝", "mem")
b += node("fault", "缺页 / 权限不足\\nhandle_page_fault(.force()) 后重试", "event")
b += '''
  src1 -> chk; src2 -> chk;
  chk -> entry -> query;
  query -> frame [label="映射存在且权限满足", color="#2F9D57"];
  query -> fault [label="需要处理", color="#C0463F"];
  fault -> query [label="重试", style=dashed, constraint=false];
'''
render("07_mem_access", wrap(b, extra='rankdir=LR, nodesep="0.45", ranksep="0.7"'))

# =====================================================================
# 8. x86-64 寄存器 ABI：字段级写策略
# =====================================================================
b  = node("trap", "tracee 陷入内核\\nsignal/syscall/exec/exit", "user")
b += node("snap", "进入 ptrace-stop\\n复制 GeneralRegs + orig_rax 到快照", "core")
b += node("rule", "CUserRegsStruct + REG_RULES\\n字段级访问策略", "sec", penwidth="2.2")
b += node("set", "rax..r15 → Set\\n自由修改", "mem")
b += node("setif", "rip/rsp/fs/gsbase → SetIf\\n必须是用户地址", "mem")
b += node("trunc", "rflags → SetBitsTruncate\\n仅用户态可控位", "mem")
b += node("fixed", "cs/ss/ds/es → Fixed\\n匹配 Linux 段不变量", "mem")
b += node("dbg", "debug regs → 读默认值\\n写返回 EOPNOTSUPP", "mem")
b += node("wb", "tracee 唤醒\\n快照写回 UserContext", "core")
b += '''
  trap -> snap -> rule;
  rule -> set; rule -> setif; rule -> trunc; rule -> fixed; rule -> dbg;
  set -> wb [style=invis]; setif -> wb; trunc -> wb [style=invis]; fixed -> wb [style=invis]; dbg -> wb [style=invis];
  { rank=same; set; setif; trunc; fixed; dbg; }
'''
render("08_register_abi", wrap(b, extra='rankdir=TB, nodesep="0.35", ranksep="0.55"'))

# =====================================================================
# 9. 断点闭环（环形流程）
# =====================================================================
steps = [
    ("b1","① maps 定位代码映射"),
    ("b2","② PEEKTEXT 读原指令"),
    ("b3","③ POKETEXT 写 int3"),
    ("b4","④ 命中 #BP → SIGTRAP"),
    ("b5","⑤ ptrace-stop 保存现场"),
    ("b6","⑥ wait 返回 SIGTRAP"),
    ("b7","⑦ GET/SETREGS 修正 RIP"),
    ("b8","⑧ 恢复原指令"),
    ("b9","⑨ SINGLESTEP 走一步"),
    ("b10","⑩ 重新写回 int3"),
    ("b11","⑪ CONT 继续"),
]
b = node("hub0", "软件断点闭环\\n（非独立模块·原语组合）", "sec", penwidth="2.4", fontsize="14")
kinds = ["proc","mem","mem","event","core","proc","core","mem","core","mem","user"]
for (nid,lab),k in zip(steps,kinds):
    b += node(nid, lab, k)
for i in range(len(steps)-1):
    b += f'  {steps[i][0]} -> {steps[i+1][0]};\n'
b += f'  {steps[-1][0]} -> {steps[3][0]} [label="再次命中", style=dashed, color="#C0463F"];\n'
b += f'  {steps[0][0]} -> hub0 [style=invis];\n'
render("09_breakpoint_loop", wrap(b, extra='nodesep="0.45", ranksep="1.1", overlap=false, mindist="1.2"'), engine="circo")

# =====================================================================
# 10. 安全模型：access check + Yama 决策流
# =====================================================================
b  = node("req", "调试请求\\nptrace attach / proc mem", "user")
b += node("same", "同进程？", "ink", shape="diamond", style="filled")
b += node("ugid", "UID/GID 匹配？\\n(Fs 或 Real creds)", "sec", shape="diamond", style="filled")
b += node("cap", "具备 CAP_SYS_PTRACE？", "sec", shape="diamond", style="filled")
b += node("yama", "Yama LSM hook", "sec")
b += node("allow", "放行", "core")
b += node("deny", "拒绝 EPERM/EACCES", "mem")
b += '''
  req -> same;
  same -> allow [label="是", color="#2F9D57"];
  same -> ugid [label="否"];
  ugid -> cap [label="否"];
  ugid -> yama [label="是"];
  cap -> yama [label="是"];
  cap -> deny [label="否", color="#C0463F"];
  yama -> allow [label="scope 通过", color="#2F9D57"];
  yama -> deny [label="scope 拒绝", color="#C0463F"];
'''
# yama scope legend
b += '''
  subgraph cluster_scope {
    label="Yama ptrace_scope"; fontname="''' + FONT + '''"; fontsize=13; fontcolor="#48227f";
    style="rounded,filled"; color="#6F42C1"; fillcolor="#F1E9FF99"; margin=12;
    y0 [label="0 Disabled · 不额外限制", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y1 [label="1 Relational（默认）· 仅祖先 / CAP", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y2 [label="2 Capability · 仅 CAP_SYS_PTRACE", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y3 [label="3 NoAttach · 全禁，设置后不可降级", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y0 -> y1 -> y2 -> y3 [style=invis];
  }
'''
render("10_security_model", wrap(b, extra='rankdir=TB, nodesep="0.45", ranksep="0.6"'))

# =====================================================================
# 11. 实现进度时间线（里程碑）
# =====================================================================
milestones = [
    ("04.23", "procfs 视图 + 安全地基", "/proc maps·mem·auxv·tid\\nforce-write · access check · Yama · tkill", "proc"),
    ("04.26", "ptrace 最小闭环", "syscall 框架 · TRACEME · CONT\\nptrace-stop · wait 整合 · exec SIGTRAP", "core"),
    ("05.14", "寄存器与单步", "GET/SETREGS · PEEK/POKEUSER\\nSINGLESTEP · 断点 · GETSIGINFO/KILL", "mem"),
    ("05.18", "options 与 event-stop", "SETOPTIONS · GETEVENTMSG\\nEXEC/EXIT event · EXITKILL", "event"),
    ("05.21", "ABI 对齐 + GDB CI", "USER_CS/SS 对齐 · debug regs 仿真\\npersonality · GDB 文档/CI", "sec"),
    ("05.28", "syscall 跟踪 + strace", "PTRACE_SYSCALL · TRACESYSGOOD\\nPEEK/POKE TEXT·DATA · strace CI", "user"),
]
b = ""
for i,(date,title,detail,k) in enumerate(milestones):
    nid=f"ms{i}"
    b += node(nid, f"{date}\\n{title}\\n{detail}", k)
# 3 列 x 2 行
b += "  ms0 -> ms1 -> ms2 [constraint=false];\n"
b += "  ms3 -> ms4 -> ms5 [constraint=false];\n"
b += "  ms2 -> ms3 [label=\"\", color=\"#5b6b7d\"];\n"
b += "  { rank=same; ms0; ms1; ms2; }\n"
b += "  { rank=same; ms3; ms4; ms5; }\n"
render("11_timeline", wrap(b, extra='rankdir=TB, nodesep="0.5", ranksep="0.9"'))

# =====================================================================
# 12. 测试与验证金字塔
# =====================================================================
b  = node("t4", "真实工具链验收\\n真实 GDB（断点/回溯/单步/改内存）· strace", "user", penwidth="2.4", fontsize="14")
b += node("t3", "兼容性测试\\ngVisor ptrace_test · 以 ABI 行为为准", "sec")
b += node("t2", "集成 / 回归测试\\ndebugger·debuggee · PTRACE_SYSCALL · proc mem/maps · Yama", "core")
b += node("t1", "单元测试\\nptrace.c · read_write_regs.c · set_options.c", "proc")
b += '''
  t1 -> t2 -> t3 -> t4 [dir=none];
'''
b += '''
  note [shape=note, fontsize=12, fontname="''' + FONT + '''", fillcolor="#E5F8EE", color="#2F9D57", style="filled",
        label="原则：以真实工具为准、以 ABI 行为为准；\\n安全测试与功能测试同等重要"];
'''
render("12_test_pyramid", wrap(b, extra='rankdir=TB, nodesep="0.4", ranksep="0.55"'))

# =====================================================================
# 13. 需求拆解：调试动作 → 内核能力
# =====================================================================
rows = [
    ("启动并追踪", "TRACEME / 父子关系", "core"),
    ("exec 后接管", "exec event / SIGTRAP", "event"),
    ("设置/命中断点", "POKETEXT + #BP→SIGTRAP", "mem"),
    ("查看/改寄存器", "GET/SETREGS · USER area", "mem"),
    ("查看/改内存", "PEEK/POKE · /proc/pid/mem", "mem"),
    ("单步/继续/跟踪", "SINGLESTEP/CONT/SYSCALL", "core"),
    ("等待状态变化", "wait/waitpid/wait4", "proc"),
    ("限制非法调试", "access check + Yama LSM", "sec"),
]
b = ""
left = ""
right = ""
for i,(a,c,k) in enumerate(rows):
    left  += "  " + node(f"a{i}", a, "user").strip() + "\n"
    right += "  " + node(f"c{i}", c, k).strip() + "\n"
    b += f"  a{i} -> c{i};\n"
b += cluster("act", "用户态调试动作", "user", left)
b += cluster("cap", "内核能力骨架", "core", right)
b += "  { rank=same; " + " ".join(f"a{i};" for i in range(len(rows))) + " }\n"
b += "  { rank=same; " + " ".join(f"c{i};" for i in range(len(rows))) + " }\n"
render("13_req_decompose", wrap(b, extra='rankdir=LR, nodesep="0.22", ranksep="1.2", compound=true'))

print("ALL DONE ->", OUT)
