# 图表重新生成说明

本目录下的 PNG 图由对应的 Graphviz `.dot` 文件生成。`.dot` 文件使用中文标签，并指定字体：

```dot
fontname="Noto Sans CJK SC"
```

如果重新生成图片时出现中文乱码、方块或缺字，请先安装中文字体：

```bash
sudo apt-get update
sudo apt-get install -y fonts-noto-cjk
fc-match "Noto Sans CJK SC"
```

然后在本目录执行：

```bash
for f in *.dot; do
  dot -Tpng "$f" -o "${f%.dot}.png"
done
```

本次已在 2026-06-16 重新生成所有 PNG。旧 PNG 已备份到 `png_backup_*` 目录。
