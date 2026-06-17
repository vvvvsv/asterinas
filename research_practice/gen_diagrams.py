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
b  = node("empty", "Empty\\n无停止信号", "ink")
b += node("pending", "Pending\\n等待 wait 观察", "event")
b += node("consumed", "Consumed\\nwait 已报告", "proc")
b += node("injected", "Injected\\ntracer 注入新信号", "mem")
b += '''
  edge [fontsize=10];
  empty -> pending [label="ptrace-stop"];
  pending -> consumed [label="wait"];
  pending -> pending [label="wait + WNOWAIT", constraint=false];
  pending -> injected [label="resume 注入"];
  consumed -> injected [label="resume 注入"];
  pending -> empty [label="resume 抑制"];
  consumed -> empty [label="resume 抑制"];
  injected -> empty [label="信号投递后清空"];
'''
render("06_signal_states", wrap(b, extra='rankdir=LR, nodesep="0.25", ranksep="0.28"'))

# =====================================================================
# 7. 跨进程内存访问：VMAR alien access（不切换页表）
# =====================================================================
b = ""
# 主干（竖直）：两入口 -> 原语 -> query -> 命中拷贝
b += node("src1", "ptrace PEEK/POKE", "user", pos="170,555")
b += node("src2", "/proc/&lt;pid&gt;/mem", "user", pos="350,555")
b += node("entry", "跨进程地址空间访问\\n统一抽象 access_alien()", "core", pos="260,485")
b += node("query", "逐页查询目标地址空间页表\\n的映射与权限（不真正切换页表）", "core", pos="260,410")
b += node("frame", "命中物理页帧\\n直接拷贝数据", "core", pos="110,300")
# 缺页侧列（右，等距向下），处理后回到 query
b += node("fcond", "页面缺失 / 权限不足", "mem", pos="520,410")
b += node("fmake", "伪造一次对该地址的缺页", "event", pos="520,350")
b += node("fhandle", "复用内核真实缺页处理\\nhandle_page_fault", "core", pos="520,280")

# 入口漏斗 -> 原语 -> query
b += "  src1:s -> entry;\n"
b += "  src2:s -> entry;\n"
b += "  entry:s -> query:n;\n"
# 命中：query 左下出，到命中物理页帧
b += '  query:w -> frame:n [color="#2F9D57"];\n'
# 未命中：query 右出，进入缺页侧列
b += '  query:e -> fcond:w [color="#C0463F"];\n'
b += '  fcond:s -> fmake:n [color="#5b6b7d"];\n'
b += '  fmake:s -> fhandle:n [color="#5b6b7d"];\n'
# 重试（实线，直角：底部回到左侧，沿命中框右侧的竖线上行进入 query 底部）
b += '  wr1 [shape=point, width=0.01, style=invis, pos="260,280"];\n'
b += '  fhandle:w -> wr1 [arrowhead=none, color="#5b6b7d"];\n'
b += '  wr1 -> query:s [color="#5b6b7d"];\n'

# 文本标签
b += tlabel("l_hit", "页表命中", "162,353", color="#1c6035")
b += tlabel("l_miss", "未命中", "410,431", color="#7d2723")
b += tlabel("l_retry", "处理完重试", "325,302", color="#3a4a5c")

dot7 = ('digraph G {\n'
  f'  graph [fontname="{FONT}", bgcolor="white", pad="0.3", splines=true];\n'
  f'  node [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.16,0.10", fontsize=13];\n'
  f'  edge [fontname="{FONT}", color="#5b6b7d", penwidth=1.4, arrowsize=0.85, fontsize=11];\n'
  + b + '}\n')
render("07_mem_access", dot7, engine="neato", extra_args=["-n1"])

# =====================================================================
# 8. x86-64 寄存器 ABI：字段级写策略
# =====================================================================
# 5 wide boxes; spacing 150, shifted right so leftmost box clears the edge
xs = [580, 730, 880, 1030, 1180]
cx = sum(xs)/len(xs)   # center x = 880
yrow = 230             # row of 5 boxes
ytop = 480             # trap
ysnap = 400
yrule = 320
ywb = 130
ybus = 180             # horizontal merge bus just below the 5 boxes

b  = node("trap", "tracee 陷入内核\\n保存用户寄存器上下文", "user", pos=f"{cx},{ytop}")
b += node("snap", "进入 ptrace-stop\\n持锁复制用户寄存器上下文到寄存器快照", "core", pos=f"{cx},{ysnap}")
b += node("rule", "tracer 持锁修改寄存器快照\\n字段级访问策略", "sec", pos=f"{cx},{yrule}")

b += node("set",   "rax..r15\\ntracer 自由修改",            "mem", pos=f"{xs[0]},{yrow}")
b += node("setif", "rip/rsp/fs/gsbase\\n必须是用户地址", "mem", pos=f"{xs[1]},{yrow}")
b += node("trunc", "rflags\\n仅能写用户态可控位", "mem", pos=f"{xs[2]},{yrow}")
b += node("fixed", "cs/ss/ds/es\\n只读, 和 Linux 一致",  "mem", pos=f"{xs[3]},{yrow}")
b += node("dbg",   "debug regs\\n只读, 未来支持写入",  "mem", pos=f"{xs[4]},{yrow}")

b += node("wb", "tracee 唤醒\\n快照写回寄存器上下文", "core", pos=f"{cx},{ywb}")

# horizontal merge bus: a point under each box + a center trunk junction
for i,x in enumerate(xs):
    b += f'  bus{i} [shape=point, width=0.01, style=invis, pos="{x},{ybus}"];\n'
b += f'  trunk [shape=point, width=0.01, style=invis, pos="{cx},{ybus}"];\n'

# spine
b += '''
  trap -> snap [color="#5b6b7d"];
  snap -> rule [color="#5b6b7d"];
'''
# fan out from rule:s to each box top
for nid in ["set","setif","trunc","fixed","dbg"]:
    b += f'  rule:s -> {nid}:n [color="#5b6b7d"];\n'
# converge: each box bottom -> its bus point (down), bus points joined into a
# horizontal bus, then one clean trunk down into wb:n
for i,nid in enumerate(["set","setif","trunc","fixed","dbg"]):
    tgt = "trunk" if nid=="trunc" else f"bus{i}"
    b += f'  {nid}:s -> {tgt} [arrowhead=none, color="#5b6b7d"];\n'
# left half of bus flows right into trunk, right half flows left into trunk
b += '  bus0 -> bus1 [arrowhead=none, color="#5b6b7d"];\n'
b += '  bus1 -> trunk [arrowhead=none, color="#5b6b7d"];\n'
b += '  bus4 -> bus3 [arrowhead=none, color="#5b6b7d"];\n'
b += '  bus3 -> trunk [arrowhead=none, color="#5b6b7d"];\n'
b += '  trunk -> wb:n [color="#5b6b7d"];\n'

dot8 = ('digraph G {\n'
  f'  graph [fontname="{FONT}", bgcolor="white", pad="0.3", splines=true];\n'
  f'  node [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.16,0.10", fontsize=13];\n'
  f'  edge [fontname="{FONT}", color="#5b6b7d", penwidth=1.4, arrowsize=0.85, fontsize=11];\n'
  + b + '}\n')
render("08_register_abi", dot8, engine="neato", extra_args=["-n1"])

# =====================================================================
# 9. 断点闭环（环形流程）
# =====================================================================
# 4×4 蛇形网格：固定坐标对齐；tracee 块=绿(core)，tracer 块=黄(user)
COL = [0, 235, 470, 705]
R1, R2, R3, R4 = 360, 240, 120, 0
T="core"; U="user"
cells = [
  ("b1", "命中 #BP\\n→ SIGTRAP",            T, COL[0], R1),
  ("b2", "ptrace-stop\\n保存现场",           T, COL[1], R1),
  ("b3", "wait(tracee)\\n返回",              U, COL[2], R1),
  ("b4", "读寄存器 rip",                     U, COL[3], R1),
  ("b5", "写寄存器\\nrip -= 1",              U, COL[3], R2),
  ("b6", "写用户空间\\n恢复原指令",            U, COL[2], R2),
  ("b7", "设置 CPU\\ntrap flag",             U, COL[1], R2),
  ("b8", "resume tracee",                   U, COL[0], R2),
  ("b9", "tracee 执行一步\\n原指令后陷入内核",  T, COL[0], R3),
  ("b10","命中 #DB\\n→ SIGTRAP",             T, COL[1], R3),
  ("b11","ptrace-stop\\n保存现场",           T, COL[2], R3),
  ("b12","wait(tracee)\\n返回",              U, COL[3], R3),
  ("b13","写用户空间\\nINT3",                U, COL[3], R4),
  ("b14","清除 CPU\\ntrap flag",             U, COL[2], R4),
  ("b15","resume tracee",                   U, COL[1], R4),
  ("b16","tracee 再次\\n走到断点",            T, COL[0], R4),
]
b = ""
for nid,lab,kind,x,y in cells:
    b += node(nid,lab,kind,pos=f"{x},{y}",width="2.1",height="0.62",fixedsize="true")
# 一次性设置断点（外接在循环之前，竖直进入 命中 #BP）；均为 tracer 操作
b += node("s1","maps 定位代码映射","user",pos="0,590",width="2.1",height="0.6",fixedsize="true")
b += node("s2","PEEKTEXT 读原指令","user",pos="0,510",width="2.1",height="0.6",fixedsize="true")
b += node("s3","POKETEXT 写 int3","user",pos="0,440",width="2.1",height="0.6",fixedsize="true")
b += tlabel("s_hdr","设置断点（一次性）","0,640",color="#7a5200")
# 图例
b += node("leg_t","tracee（被调试程序）执行","core",pos="705,585",width="2.6",height="0.44",fixedsize="true",fontsize="11")
b += node("leg_r","tracer（调试器）操作","user",pos="705,520",width="2.6",height="0.44",fixedsize="true",fontsize="11")
# 设置断点链 -> 进入循环
b += '  s1:s -> s2:n [color="#5b6b7d"];\n'
b += '  s2:s -> s3:n [color="#5b6b7d"];\n'
b += '  s3:s -> b1:n [color="#5b6b7d"];\n'
# 顺序流（灰，蛇形）
seq_edges = [
  ("b1:e","b2:w"),("b2:e","b3:w"),("b3:e","b4:w"),
  ("b4:s","b5:n"),
  ("b5:w","b6:e"),("b6:w","b7:e"),("b7:w","b8:e"),
  ("b8:s","b9:n"),
  ("b9:e","b10:w"),("b10:e","b11:w"),("b11:e","b12:w"),
  ("b12:s","b13:n"),
  ("b13:w","b14:e"),("b14:w","b15:e"),("b15:w","b16:e"),
]
for a,c in seq_edges:
    b += f'  {a} -> {c} [color="#5b6b7d"];\n'
# 回到开头（红实线，走最左侧竖线）
b += '  lb1 [shape=point, width=0.01, style=invis, pos="-130,0"];\n'
b += '  lb2 [shape=point, width=0.01, style=invis, pos="-130,360"];\n'
b += '  b16:w -> lb1 [arrowhead=none, color="#C0463F"];\n'
b += '  lb1 -> lb2 [arrowhead=none, color="#C0463F"];\n'
b += '  lb2 -> b1:w [color="#C0463F"];\n'
b += tlabel("fb","再次命中","-130,185",color="#C0463F")

dot9 = ('digraph G {\n'
  f'  graph [fontname="{FONT}", bgcolor="white", pad="0.3", splines=true];\n'
  f'  node [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.1,0.06", fontsize=12];\n'
  f'  edge [fontname="{FONT}", color="#5b6b7d", penwidth=1.4, arrowsize=0.85, fontsize=11];\n'
  + b + '}\n')
render("09_breakpoint_loop", dot9, engine="neato", extra_args=["-n1"])

# =====================================================================
# 10. 安全模型：access check + Yama 决策流
# =====================================================================
b  = '  node [margin="0.1,0.035"];\n'
b += node("req", "调试请求\\nptrace attach / proc mem", "user")
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
render("10_security_model", wrap(b, extra='rankdir=TB, nodesep="0.22", ranksep="0.32"'))

# =====================================================================
# 11. 实现进度时间线（里程碑）
# =====================================================================
# ---- data ----
ms = [
 ("ms0","04.23","procfs 视图 + 安全地基","/proc maps、mem、tid 等\\nforce-write、access check、Yama、tkill","proc"),
 ("ms1","04.26","ptrace 最小闭环","syscall 框架、TRACEME、CONT\\nptrace-stop、wait 整合、exec SIGTRAP","core"),
 ("ms2","05.14","寄存器与单步","GET/SETREGS、PEEK/POKEUSER\\nSINGLESTEP、断点、GETSIGINFO/KILL","mem"),
 ("ms3","05.18","options 与 event-stop","SETOPTIONS、GETEVENTMSG\\nEXEC/EXIT event、EXITKILL","event"),
 ("ms4","05.21","ABI 对齐 + GDB CI","USER_CS/SS 对齐、debug regs 仿真\\npersonality、GDB 文档/CI","sec"),
 ("ms5","05.28","syscall 跟踪 + strace","PTRACE_SYSCALL、TRACESYSGOOD\\nPEEK/POKE TEXT、DATA、strace CI","user"),
]

# ---- 2-row serpentine layout (points, y-up) ----
COL_DX = 345
X0 = 210
ROW_TOP = 405
ROW_BOT = 150
CARD_W = 2.30
CARD_HALF_W = 172   # approx half card width in points (for routing)

# true serpentine: top row ms0->ms1->ms2 (L->R);
# bottom row ms3->ms4->ms5 flows R->L, so ms3 sits under ms2 (right side).
pos = {}
for i in range(3):
    pos[i] = (X0 + COL_DX*i, ROW_TOP)
# bottom row laid out so ms3 under ms2, ms4 under ms1, ms5 under ms0
for i,col in [(3,2),(4,1),(5,0)]:
    pos[i] = (X0 + COL_DX*col, ROW_BOT)

b = ""
for i,(mid,date,title,detail,kind) in enumerate(ms):
    x,y = pos[i]
    label = f"{date}\\n{title}\\n{detail}"
    b += node(mid,label,kind, pos=f"{x},{y}", width=str(CARD_W))

# straight flow arrows within rows
def edge(a,bn,color,head="normal"):
    return f'  {a} -> {bn} [color="{color}", penwidth=2.2, arrowsize=1.0, dir={head}];\n'

flow_col = "#7a8aa0"
b += edge("ms0","ms1",flow_col)
b += edge("ms1","ms2",flow_col)
# wrap arrow ms2 (top right) -> ms3 (bottom right): clean straight vertical drop.
x2,_ = pos[2]
b += f'  ms2 -> ms3 [color="{flow_col}", penwidth=2.4, arrowsize=1.1];\n'
# bottom row flows right->left: ms3 -> ms4 -> ms5
b += edge("ms3","ms4",flow_col)
b += edge("ms4","ms5",flow_col)

# wrap-arrow text box (beside the vertical drop, between the two rows)
wrap_y = (ROW_TOP+ROW_BOT)/2
b += tlabel("wraptxt","时间推进", f"{x2+62},{wrap_y}")

dot11 = ('digraph G {\n'
  f'  graph [fontname="{FONT}", bgcolor="white", pad="0.3", splines=true];\n'
  f'  node [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.16,0.10", fontsize=12];\n'
  f'  edge [fontname="{FONT}", color="#5b6b7d", penwidth=1.6, arrowsize=0.9, fontsize=11];\n'
  + b + '}\n')
render("11_timeline", dot11, engine="neato", extra_args=["-n1"])

# =====================================================================
# 12. 测试与验证金字塔
# =====================================================================
b=""
b+=node("apex","真实工具链验收\\n真实 GDB（断点/回溯/单步/改内存）、strace","user",
        pos="0,246", width="4.4", height="0.92", fixedsize="true", penwidth="2.4")
b+=node("t3","兼容性测试\\ngVisor ptrace_test、以 ABI 行为为准","sec",
        pos="0,164", width="5.0", height="0.92", fixedsize="true")
b+=node("t2","集成 / 回归测试\\ndebugger、debuggee、PTRACE_SYSCALL、\\nproc mem/maps、Yama","core",
        pos="0,82", width="5.7", height="1.05", fixedsize="true")
b+=node("base","单元测试\\nptrace.c、read_write_regs.c、set_options.c","proc",
        pos="0,0", width="6.4", height="0.92", fixedsize="true")

# right column: note (beside narrow tier), annotations near apex/base
b+=tlabel("ann_top","少、慢、高价值", "255,246", "#7a5200")
b+=('  note1 [shape=note, style="filled", fillcolor="#E5F8EE", color="#2F9D57", '
    'fontsize=11, fontcolor="#1c6035", pos="273,158", '
    'label="原则：以真实工具为准、\\n以 ABI 行为为准；\\n安全测试与功能测试\\n同等重要"];\n')
b+=tlabel("ann_bot","多、快、廉价", "255,0", "#0f5151")

dot12 = ('digraph G {\n'
  f'  graph [fontname="{FONT}", bgcolor="white", pad="0.3", splines=true];\n'
  f'  node [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.16,0.12", fontsize=13];\n'
  f'  edge [fontname="{FONT}", color="#5b6b7d", penwidth=1.4, arrowsize=0.85, fontsize=11];\n'
  + b + '}\n')
render("12_test_pyramid", dot12, engine="neato", extra_args=["-n1"])

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

# =====================================================================
# 4b. tracer / tracee 同步：TraceeStatus 上的一把锁 + is_stopped 原子标志
#     固定坐标（neato -n1）：双泳道 + 中间共享对象
# =====================================================================
b = ""

def fnode(nid,label,kind,pos,w,h=0.62,fs=12,bold=False):
    fill,border,font=PAL[kind]
    style="rounded,filled,bold" if bold else "rounded,filled"
    return (f'  {nid} [label="{label}", fillcolor="{fill}", color="{border}", fontcolor="{font}", '
            f'shape=box, style="{style}", fixedsize=true, width="{w}", height="{h}", '
            f'fontsize="{fs}", pos="{pos}"];\n')

def fbox(nid, lines, kind, pos, w, h, fs=11):
    # 固定大小圆角框；HTML 表格，整体居中；第一行（标题）加粗
    fill,border,fcol=PAL[kind]
    rows=""
    for i,ln in enumerate(lines):
        t=ln.replace("&","&amp;").replace("<","&lt;").replace(">","&gt;")
        if i==0: t=f"<B>{t}</B>"
        rows+=f'<TR><TD ALIGN="CENTER"><FONT POINT-SIZE="{fs}">{t}</FONT></TD></TR>'
    lbl='<<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="2">'+rows+'</TABLE>>'
    return (f'  {nid} [label={lbl}, fillcolor="{fill}", color="{border}", fontcolor="{fcol}", '
            f'fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, '
            f'fixedsize=true, width="{w}", height="{h}", pos="{pos}"];\n')

def cont(nid, pos, w, h, border):
    # 大容器框：白底、彩色粗边、圆角；先声明（在底层），小块画在其上
    return (f'  {nid} [label="", fillcolor="white", color="{border}", shape=box, '
            f'style="rounded,filled", penwidth=2.4, fixedsize=true, width="{w}", height="{h}", pos="{pos}"];\n')

def gtitle(nid, text, pos, color, fs=14):
    return (f'  {nid} [shape=plaintext, style="filled", fillcolor="white", '
            f'fontname="{FONT}", fontcolor="{color}", '
            f'fontsize="{fs}", label=<<B>{text}</B>>, pos="{pos}"];\n')

# 三个大框：tracee 小块 | 中间共享对象 | tracer 小块；左右两框连边到中间框
# 容器先声明（底层），随后标题与小块画在其上
b += cont("tracee_grp","200,372",3.6,5.6,"#2F9D57")
b += cont("center_grp","500,372",3.6,2.35,"#6F42C1")
b += cont("tracer_grp","800,372",3.6,5.6,"#C8881A")

b += gtitle("tracee_t","tracee 线程","200,552","#1c6035")
b += gtitle("center_t","TraceeStatus 共享对象","500,435","#48227f")
b += gtitle("tracer_t","tracer 线程","800,552","#7a5200")

# --- TRACEE 小块（左框内，等宽 TW） ---
TW=3.3
b += fbox("t_stop",["进入 ptrace-stop","获取锁","保存信号、寄存器、event 到锁内","置 is_stopped = true","释放锁"],"core","200,470",TW,1.42,11)
b += fbox("t_park",["挂起，等待 tracer","pause_until(!is_stopped)","SIGKILL 可随时打断"],"core","200,345",TW,0.92,11)
b += fbox("t_wake",["被唤醒，继续运行","获取锁，读回快照","释放锁"],"core","200,238",TW,0.92,11)

# --- TRACER 小块（右框内，等宽 RW） ---
RW=3.4
b += fbox("r_wait",["wait 报告","获取锁","取 wait4 状态后释放锁"],"user","800,488",RW,0.92,11)
b += fbox("r_inspect",["读 / 改寄存器、内存","获取锁，检查 is_stopped","读 / 写寄存器快照、读 / 写用户空间","释放锁"],"user","800,360",RW,1.15,11)
b += fbox("r_resume",["resume","获取锁","注入 / 清除待决信号","置 is_stopped = false","释放锁"],"user","800,228",RW,1.42,11)

# --- CENTER 共享对象（中间框内） ---
CW=3.3
b += fbox("c_mtx",["state: Mutex<TraceeState>","寄存器快照、待决信号、event、options"],"sec","500,378",CW,0.72,11)
b += fbox("c_atom",["is_stopped: AtomicBool"],"sec","500,323",CW,0.46,11)

# ============ EDGES ============
# 各线程内部顺序流
b += '  t_stop:s -> t_park:n [color="#2F9D57"];\n'
b += '  r_wait:s -> r_inspect:n [color="#C8881A"];\n'
b += '  r_inspect:s -> r_resume:n [color="#C8881A"];\n'
# 左右大框 <-> 中间共享对象
b += '  tracee_grp:e -> center_grp:w [dir=both, color="#6F42C1", penwidth=2.2];\n'
b += '  tracer_grp:w -> center_grp:e [dir=both, color="#6F42C1", penwidth=2.2];\n'
# SIGCHLD：tracee 释放锁后通知 tracer（绕中间框上方）
b += '  wsig [shape=point, width=0.01, style=invis, pos="500,538"];\n'
b += '  t_stop:e -> wsig [color="#2F9D57", arrowhead=none];\n'
b += '  wsig -> r_wait:w [color="#2F9D57"];\n'
b += tlabel("L_sig","向tracer发送 SIGCHLD\n唤醒挂起在 wait 的 tracer","500,525","#1c6035")
# 唤醒：tracer resume 唤醒 tracee（绕中间框下方，用 tracer 的颜色）
b += '  wwake [shape=point, width=0.01, style=invis, pos="500,188"];\n'
b += '  r_resume:w -> wwake [color="#C8881A", arrowhead=none];\n'
b += '  wwake -> t_wake:e [color="#C8881A"];\n'
b += tlabel("L_wk","唤醒挂起在 ptrace stop 的 tracee","500,202","#7a5200")

dotS = ('digraph G {\n'
  f'  graph [fontname="{FONT}", bgcolor="white", pad="0.3", splines=true];\n'
  f'  node [fontname="{FONT}", shape=box, style="rounded,filled", penwidth=1.5, margin="0.16,0.10", fontsize=12];\n'
  f'  edge [fontname="{FONT}", color="#5b6b7d", penwidth=1.4, arrowsize=0.85, fontsize=11];\n'
  + b + '}\n')
render("04b_sync", dotS, engine="neato", extra_args=["-n1"])

print("ALL DONE ->", OUT)

