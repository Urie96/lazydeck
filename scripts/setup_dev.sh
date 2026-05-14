#!/usr/bin/env bash
# 生成 .luarc.json 配置文件，包含所有 preset 和 plugins 的绝对路径

set -e

# 获取脚本所在目录的绝对路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PRESET_DIR="$PROJECT_ROOT/preset/lua"
PLUGINS_DIR="$HOME/.local/share/lazydeck/plugins"
LUARC_FILE="${PLUGINS_DIR}/.luarc.json"

# 收集所有路径
declare -a paths=()

# 添加 preset/lua 目录下的所有 .lua 文件
while IFS= read -r file; do
  [[ -n "$file" ]] || continue
  paths+=("$file")
done < <(find "$PRESET_DIR" -name "*.lua" -type f 2>/dev/null | sort)

# 添加 plugins 目录下所有 .lazydeck 目录（只需要目录，不需要单独列文件）
while IFS= read -r dir; do
  [[ -n "$dir" ]] || continue
  paths+=("$dir/")
done < <(ls -d "$PLUGINS_DIR"/*.lazydeck 2>/dev/null)

# 生成 .luarc.json
{
  echo '{'
  echo '    "runtime.version": "Lua 5.4",'
  echo '    "workspace.library": ['

  first=true
  for p in "${paths[@]}"; do
    if $first; then
      echo "        \"$p\""
      first=false
    else
      echo "        ,\"$p\""
    fi
  done

  echo '    ]'
  echo '}'
} >"$LUARC_FILE"

echo "✓ 已生成 $LUARC_FILE"
echo "  - ${#paths[@]} 个路径已配置"

# Create symlinks for config and plugins directories
# Remove existing regular directories (not symlinks) before creating links
[[ -d config && ! -L config ]] && rm -rf config
[[ -d plugins && ! -L plugins ]] && rm -rf plugins

[[ ! -e config ]] && ln -s ~/.config/lazydeck/ ./config
[[ ! -e plugins ]] && ln -s ~/.local/share/lazydeck/plugins/ ./plugins
