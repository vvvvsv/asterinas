# Asterinas 支持 RustyVault

## 环境配置

image 为 asterinas/asterinas:0.14.1-20250326

[Asterinas 源码分支](https://github.com/vvvvsv/asterinas/tree/support_rvault)
[RustyVault 源码分支](https://github.com/vvvvsv/RustyVault/tree/main)

```bash
apt update
apt install pkg-config libssl-dev
```

## 对 RustyVault 的修改

核心逻辑保持不变。为了在 Asterinas 上运行，做了如下修改：

1. 使用 fork 绕过了 daemonize crate
2. SystemMetrics 修改为 dummy 实现，不依赖 sysinfo crate，因为 Asterinas 的 procfs 缺失太多了

## 运行 demo

```bash
make run RELEASE=1
```

```bash
cd test/rvault
./rvault server --config rvault.hcl

cd ../rvault_tests
./0_init_rvault
./1_check_init
./2_unseal <keys>
./3_write_secret <root_token>
./3_read_secret <root_token>
```

预期响应和 [RustyVault 快速开始](https://rustyvault.net/zh-CN/docs/quick-start/) 中的例子相似。

```
   _   ___ _____ ___ ___ ___ _  _   _   ___
  /_\ / __|_   _| __| _ \_ _| \| | /_\ / __|
 / _ \\__ \ | | | _||   /| || .` |/ _ \\__ \
/_/ \_\___/ |_| |___|_|_\___|_|\_/_/ \_\___/


~ # cd test/rvault
/test/rvault #
/test/rvault # ./rvault server --config rvault.hcl
/test/rvault #
/test/rvault # cd ../rvault_tests
/test/rvault_tests #
/test/rvault_tests # ./0_init_rvault
HTTP/1.1 200 OK
content-length: 129
connection: close
content-type: application/json
date: Sun, 27 Jul 2025 13:56:32 GMT

{"keys":["d4a661038e3f14a5bce675dee0d8a754e4af7b05f300cc7bb3e1b6cb14bded5f"],"root_token":"0eb8149d-85c1-43db-8e90-b166bfde06d3"}
/test/rvault_tests # ./1_check_init
HTTP/1.1 200 OK
content-length: 20
connection: close
content-type: application/json
date: Sun, 27 Jul 2025 13:56:35 GMT

{"initialized":true}
/test/rvault_tests # ./2_unseal d4a661038e3f14a5bce675dee0d8a754e4af7b05f300cc7b
b3e1b6cb14bded5f
HTTP/1.1 200 OK
content-length: 41
connection: close
content-type: application/json
date: Sun, 27 Jul 2025 13:56:41 GMT

{"sealed":false,"t":1,"n":1,"progress":0}
/test/rvault_tests # ./3_write_secret 0eb8149d-85c1-43db-8e90-b166bfde06d3
HTTP/1.1 204 No Content
connection: close
date: Sun, 27 Jul 2025 13:56:48 GMT


/test/rvault_tests # ./4_read_secret 0eb8149d-85c1-43db-8e90-b166bfde06d3
HTTP/1.1 200 OK
content-length: 113
connection: close
content-type: application/json
date: Sun, 27 Jul 2025 13:56:51 GMT

{"renewable":false,"lease_id":"","lease_duration":3600,"auth":null,"data":{"Asterinas":"RustyVault","foo":"bar"}}
/test/rvault_tests #
```