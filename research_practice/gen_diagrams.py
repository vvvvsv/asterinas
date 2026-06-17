#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate polished framework diagrams (.dot -> .png) for the defense deck."""
import os, subprocess, textwrap, math

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

def render(name, dot, engine="dot", extra_args=None):
    path = os.path.join(OUT, name + ".dot")
    with open(path, "w") as f:
        f.write(dot)
    cmd = ["/opt/homebrew/bin/dot", f"-K{engine}", "-Tpng", "-Gdpi=150"]
    if extra_args:
        cmd += extra_args
    cmd += [path, "-o", os.path.join(OUT, name + ".png")]
    subprocess.run(cmd, check=True)
    print("rendered", name)

def wrap(dotbody, extra='rankdir=TB, nodesep="0.5", ranksep="0.6"'):
    return "digraph G {\n" + HEADER.replace("{extra}", extra) + dotbody + "}\n"

# =====================================================================
# 1. 总体架构：固定坐标（neato -n），位置写死；紧凑布局
# =====================================================================
def _tbl(inner, border):
    return (f'<TABLE BORDER="0" CELLBORDER="1" COLOR="{border}" '
            f'CELLSPACING="3" CELLPADDING="3">{inner}</TABLE>')

def hblock(nid, title, kind, rows, pos):
    fill, border, font = PAL[kind]
    trs = ""
    for row in rows:
        tds = ""
        for cell in row:
            tds += (f'<TD BGCOLOR="{fill}"><FONT POINT-SIZE="11" COLOR="{font}">'
                    + cell.replace("\n", "<BR/>") + "</FONT></TD>")
        trs += f"<TR>{tds}</TR>"
    label = ('<<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="0">'
             f'<TR><TD CELLPADDING="2"><FONT POINT-SIZE="13" COLOR="{font}"><B>{title}</B></FONT></TD></TR>'
             f'<TR><TD>{_tbl(trs, border)}</TD></TR></TABLE>>')
    return (f'  {nid} [shape=box, style="rounded,filled", fillcolor="{fill}40", '
            f'color="{border}", penwidth=1.6, pos="{pos}", label={label}];\n')

def smblock(pos):
    fill, border, font = PAL["core"]
    cells = [("建立关系", "TRACEME / attach", ""), ("ptrace-stop", "保存现场, 阻塞", 'PORT="pstop"'),
             ("wait 报告", "SIGCHLD &#8594; wait4", ""), ("resume", "CONT, STEP, SYSCALL", 'PORT="pres"')]
    row = ""
    for i, (a, c, port) in enumerate(cells):
        row += f'<TD {port} BGCOLOR="{fill}"><FONT POINT-SIZE="11" COLOR="{font}">{a}<BR/>{c}</FONT></TD>'
        if i < len(cells) - 1:
            row += f'<TD BORDER="0"><FONT POINT-SIZE="12" COLOR="{font}">&#8594;</FONT></TD>'
    spacer = f'<TR><TD COLSPAN="7" BORDER="0" HEIGHT="30"><FONT POINT-SIZE="11" COLOR="{font}">                              再停止（resume 后命中新 stop）</FONT></TD></TR>'
    label = ('<<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="0">'
             f'<TR><TD CELLPADDING="2"><FONT POINT-SIZE="13" COLOR="{font}"><B>④ tracer / tracee 状态机</B></FONT></TD></TR>'
             f'<TR><TD>{_tbl(f"<TR>{row}</TR>" + spacer, border)}</TD></TR></TABLE>>')
    return (f'  sm [shape=box, style="rounded,filled", fillcolor="{fill}33", '
            f'color="{border}", penwidth=1.6, pos="{pos}", label={label}];\n')

b = ""
b += hblock("user", "① USER APP（GDB / strace）", "user", [["GDB 调试器", "strace 跟踪器"]], "0,500")
b += hblock("sys", "② 系统调用接口（Linux ABI）", "sys",
            [["sys_ptrace\n系统调用分发", "/proc 文件接口\nmaps, mem, ..."]], "0,400")
b += hblock("perm", "③ 权限检查（统一鉴权）", "sec",
            [["Yama LSM\nptrace_scope 策略", "alien access 凭证\nRead/Attach, Fs/Real"]], "250,360")
b += smblock("0,258")
b += hblock("alien", "⑤ 跨进程用户空间读写", "mem",
            [["底层实现\nVMAR alien access", "API\nptrace PEEK / POKE\n/proc/pid/mem"]], "-170,120")
b += hblock("reg", "⑥ 寄存器上下文快照", "proc",
            [["CUserRegsStruct\nUSER area", "字段级写策略\nrip/rsp/段寄存器 等"]], "80,120")
b += hblock("cowork", "⑦ 与其他系统调用协作", "event",
            [["signal", "wait", "clone"], ["exec", "exit", "fork"]], "285,120")
b += '  gate [shape=point, width=0.03, color="#5b6b7d", pos="0,340"];\n'
# 「鉴权放行」改成手动定位的文字节点（改下面 pos 的 x,y 即可挪动）
b += '  authlbl [shape=plaintext, pos="80,340", fontsize=12, fontcolor="#48227f", label="鉴权放行"];\n'
# 再停止的两个途径点（改 pos 调整曲线形状）
b += '  wp1 [shape=point, width=0.01, style=invis, pos="180,215"];\n'
b += '  wp2 [shape=point, width=0.01, style=invis, pos="-75,215"];\n'

b += '''
  user -> sys [penwidth=2.6, color="#5b6b7d"];
  sys -> gate [arrowhead=none, penwidth=2.6, color="#5b6b7d"];
  gate -> sm [penwidth=2.6, color="#5b6b7d"];
  perm:w -> gate [color="#6F42C1"];
  sm -> alien [penwidth=2.2, color="#5b6b7d"];
  sm -> reg [penwidth=2.2, color="#5b6b7d"];
  sm -> cowork [penwidth=2.2, color="#5b6b7d"];
  sm:pres:s -> wp1 [arrowhead=none, style=dashed, color="#2F9D57"];
  wp1 -> wp2 [arrowhead=none, style=dashed, color="#2F9D57"];
  wp2 -> sm:pstop:s [style=dashed, color="#2F9D57"];
'''
dot1 = ('digraph G {\n'
        f'  graph [fontname="{FONT}", bgcolor="white", pad="0.25", splines=true];\n'
        f'  node [fontname="{FONT}"];\n'
        f'  edge [fontname="{FONT}", arrowsize=0.85];\n'
        + b + '}\n')
render("01_arch_overview", dot1, engine="neato", extra_args=["-n1"])

# =====================================================================
# 2. ptrace 请求分发：入口薄、状态机厚
# =====================================================================
b  = node("g", "GDB / strace\\nptrace(request, tid, addr, data)", "user")
b += node("p", "sys_ptrace 解析 PtraceRequest", "sys")
b += node("attach", "TRACEME\\n建立 tracer/tracee 关系\\n+ 权限检查", "sec")
b += node("look", "get_tracee(tid)\\n校验追踪关系", "core")
b += node("mem", "PEEK/POKE TEXT, DATA\\n读写 tracee 内存", "mem")
b += node("ua",  "PEEK/POKE USER, GET/SETREGS\\n寄存器与 USER area", "mem")
b += node("cont","CONT, SINGLESTEP, SYSCALL\\n注入信号 + 恢复运行", "core")
b += node("opt", "SETOPTIONS, GETEVENTMSG, GETSIGINFO\\n事件与 siginfo", "event")
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
b += node("status", "TraceeStatus\\nis_stopped, state", "core")
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
# 4. ptrace-stop 主状态机（生命周期 + 三类停止统一收敛）
#    固定坐标（neato -n1）：改下面每个节点的 pos="x,y" 即可挪动方框
#    线上不写 label，所有文字都用单独的白底文本框（tlabel），位置可单独微调
# =====================================================================
def tlabel(nid, text, pos, color="#3a4a5c"):
    return (f'  {nid} [shape=box, style="filled", fillcolor="white", color="white", '
            f'penwidth=0, margin="0.03,0.01", fontsize=11, fontcolor="{color}", '
            f'pos="{pos}", label="{text}"];\n')

# 生命周期主干（圆角矩形）+ 三类停止来源，全部写死坐标
b  = node("untraced", "未被跟踪", "ink", pos="0,360")
b += node("attached", "已建立 trace 关系", "user", pos="0,240")
b += node("sig", "信号投递停\\nsignal-delivery-stop\\n不拦截 SIGKILL", "event", pos="180,320")
b += node("sys", "系统调用停\\nsyscall-stop (entry/exit)\\nPTRACE_SYSCALL", "event", pos="180,240")
b += node("evt", "ptrace 事件停\\nptrace-event-stop\\nPTRACE_SETOPTIONS", "event", pos="180,160")
b += node("stopped",
          "ptrace-stop\\n统一抽象 do_ptrace_stop()\\n保存寄存器现场\\n记录信号、事件、wait status\\n投递 SIGCHLD 通知 tracer\\n阻塞 tracee，等待 tracer 指示",
          "core", penwidth="2.6", fontsize="14", pos="420,240")
b += node("reported", "tracer 调用 wait 报告\\nLinux 风格 wait4 状态字", "proc", pos="740,380")
b += node("running", "resume 恢复运行\\n回写寄存器快照\\n按需设置单步执行", "user", pos="740,240")
b += node("exited", "退出 / 清理\\nTraceeExit + detach", "mem", pos="740,120")
# 「下一次停止条件」回环的中间途径点（改 pos 调整这条线的高度/弯度）
b += '  loopwp [shape=point, width=0.01, style=invis, pos="600,240"];\n'
# 线上的文字 —— 单独文本框，改各自 pos 即可移动
b += tlabel("t_attach",  "TRACEME / attach", "0,300")
b += tlabel("t_sigchld", "SIGCHLD 唤醒", "585,325")
b += tlabel("t_cont",    "tracer continue", "740,318")
b += tlabel("t_loop",    "下一次\n停止条件", "595,240")
b += tlabel("t_kill",    "SIGKILL 打断", "585,160")
b += tlabel("t_exit",    "exit", "740,180")

b += '''
  untraced -> attached;
  attached -> sig:w  [color="#9A9620"];
  attached -> sys:w  [color="#9A9620"];
  attached -> evt:w  [color="#9A9620"];
  sig:e -> stopped [color="#9A9620"];
  sys:e -> stopped [color="#9A9620"];
  evt:e -> stopped [color="#9A9620"];
  stopped:ne -> reported;
  reported -> running;
  running:w -> loopwp [arrowhead=none];
  loopwp -> stopped:e;
  stopped:se -> exited;
  running -> exited;
'''
dot4 = ('digraph G {\n'
        f'  graph [fontname="{FONT}", bgcolor="white", pad="0.3", splines=true];\n'
        f'  node [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.16,0.10", fontsize=13];\n'
        f'  edge [fontname="{FONT}", color="#5b6b7d", penwidth=1.4, arrowsize=0.85, fontsize=11];\n'
        + b + '}\n')
render("04_state_machine", dot4, engine="neato", extra_args=["-n1"])

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
b = node("hub0", "软件断点闭环\\n（非独立模块、原语组合）", "sec", penwidth="2.4", fontsize="14")
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
    y0 [label="0 Disabled、不额外限制", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y1 [label="1 Relational（默认）、仅祖先 / CAP", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y2 [label="2 Capability、仅 CAP_SYS_PTRACE", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y3 [label="3 NoAttach、全禁，设置后不可降级", fillcolor="#F1E9FF", color="#6F42C1", fontname="''' + FONT + '''", fontsize=12];
    y0 -> y1 -> y2 -> y3 [style=invis];
  }
'''
render("10_security_model", wrap(b, extra='rankdir=TB, nodesep="0.45", ranksep="0.6"'))

# =====================================================================
# 11. 实现进度时间线（里程碑）
# =====================================================================
milestones = [
    ("04.23", "procfs 视图 + 安全地基", "/proc maps, mem, tid 等\\nforce-write, access check, Yama, tkill", "proc"),
    ("04.26", "ptrace 最小闭环", "syscall 框架、TRACEME, CONT\\nptrace-stop, wait 整合、exec SIGTRAP", "core"),
    ("05.14", "寄存器与单步", "GET/SETREGS, PEEK/POKEUSER\\nSINGLESTEP、断点、GETSIGINFO/KILL", "mem"),
    ("05.18", "options 与 event-stop", "SETOPTIONS, GETEVENTMSG\\nEXEC/EXIT event, EXITKILL", "event"),
    ("05.21", "ABI 对齐 + GDB CI", "USER_CS/SS 对齐、debug regs 仿真\\npersonality, GDB 文档/CI", "sec"),
    ("05.28", "syscall 跟踪 + strace", "PTRACE_SYSCALL, TRACESYSGOOD\\nPEEK/POKE TEXT, DATA, strace CI", "user"),
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
b  = node("t4", "真实工具链验收\\n真实 GDB（断点/回溯/单步/改内存）、strace", "user", penwidth="2.4", fontsize="14")
b += node("t3", "兼容性测试\\ngVisor ptrace_test、以 ABI 行为为准", "sec")
b += node("t2", "集成 / 回归测试\\ndebugger, debuggee, PTRACE_SYSCALL, proc mem/maps, Yama", "core")
b += node("t1", "单元测试\\nptrace.c, read_write_regs.c, set_options.c", "proc")
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
    ("查看/改寄存器", "GET/SETREGS, USER area", "mem"),
    ("查看/改内存", "PEEK/POKE, /proc/pid/mem", "mem"),
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

# =====================================================================
# 0. 研究背景：需求（上）-> 调试能力（中心）-> 一圈内核子系统（真、环形）
# =====================================================================
ring = [   # (id, label, kind, angle°)  —— 跳过正上方 90°，留给需求箭头
    ("r3", "wait/waitpid\\n状态报告", "proc", 0),       # 右
    ("r2", "ptrace 状态机\\n信号拦截 + 投递", "event", 45),  # 右上
    ("r7", "procfs 视图\\nmaps, mem, ...", "proc", 135),  # 左上
    ("r6", "安全策略\\n权限检查, Yama", "sec", 180),       # 左
    ("r5", "寄存器上下文\\nuser_regs, 单步", "mem", 225),  # 左下
    ("r4", "地址空间\\nVMAR 跨进程", "mem", 270),         # 下
    ("r1", "进程管理\\nfork, exec, exit", "sys", 315),    # 右下
]
R = 185.0                     # 环半径（pt）
YS = 0.72                      # 纵向压扁，减少竖向留白
parts = ['digraph G {',
         f'  graph [fontname="{FONT}", bgcolor="white", pad="0.2", splines=true];',
         f'  node  [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.13,0.07", fontsize=13];',
         f'  edge  [fontname="{FONT}", penwidth=1.5, arrowsize=0.9];']
# 中心 + 需求（正上方）
parts.append(node("hub", "进程调试能力", "core", penwidth="2.8", fontsize="18", pos='0,0'))
parts.append(node("need", "面向开发者的操作系统\\n需要支持用户态调试", "user",
                  penwidth="2.4", fontsize="14", pos=f'0,{R*YS+50:.0f}'))
for (nid, lab, k, a) in ring:
    x = -R * math.cos(math.radians(a))   # 取负：左右镜像 → 旋转方向倒转
    y = R * math.sin(math.radians(a)) * YS
    parts.append(node(nid, lab, k, pos=f'{x:.0f},{y:.0f}'))
parts.append(f'  need -> hub [color="#C8881A", penwidth=2.6];')
for (nid, _, _, _) in ring:
    parts.append(f'  hub -> {nid} [color="#2F9D57"];')
parts.append('}')
render("00_background", "\n".join(parts), engine="neato", extra_args=["-n1"])

print("ALL DONE ->", OUT)

