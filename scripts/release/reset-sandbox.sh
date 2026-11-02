#!/usr/bin/env bash
#
# 重置 MonoSub 的沙盒授权，用于反复验证「授权一次 → 之后不再问」。
#
#   ./scripts/release/reset-sandbox.sh              # 只清目录授权（保留设置/日志/缓存）
#   ./scripts/release/reset-sandbox.sh --full       # 整个容器清空，等价于首次安装
#   ./scripts/release/reset-sandbox.sh --dry-run    # 只打印将要做什么
#
# 三类"授权"要分清，改错地方等于白忙：
#
#   目录授权（保存位置 / 模型目录）  security-scoped bookmark，存在 settings.json 的
#                                    dirGrants 里 —— 由本脚本清理，和 TCC 无关
#   通知/麦克风/屏幕录制等隐私项     TCC 数据库 —— 用 tccutil reset，本脚本顺带清
#   沙盒容器本身                     ~/Library/Containers/<bundle id> —— 删掉等于重装，
#                                    bookmark 会全部失效（容器 UUID 变了）

set -euo pipefail

BUNDLE_ID="leixlabs.monosub"
APP_NAME="MonoSub"
CONTAINER="$HOME/Library/Containers/$BUNDLE_ID"
SETTINGS="$CONTAINER/Data/Library/Application Support/$BUNDLE_ID/settings.json"
EXPECTED_CONTAINER="$HOME/Library/Containers/$BUNDLE_ID"

MODE="grants"
DRY_RUN=0
ASSUME_YES=0

for arg in "$@"; do
  case "$arg" in
    --grants) MODE="grants" ;;
    --full) MODE="full" ;;
    --dry-run) DRY_RUN=1 ;;
    --yes|-y) ASSUME_YES=1 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "未知参数：${arg}（--grants | --full | --dry-run | --yes）" >&2; exit 2 ;;
  esac
done

say() { printf '\033[36m▸\033[0m %s\n' "$*"; }
warn() { printf '\033[33m!\033[0m %s\n' "$*"; }
die() { printf '\033[31m✗\033[0m %s\n' "$*" >&2; exit 1; }

# --- 0. 安全检查：只允许动我们自己的容器 -------------------------------------
[ "$CONTAINER" = "$EXPECTED_CONTAINER" ] || die "容器路径异常：$CONTAINER"
case "$CONTAINER" in
  "$HOME/Library/Containers/"*) : ;;
  *) die "拒绝操作容器之外的路径：$CONTAINER" ;;
esac

# --- 1. 先让 app 退出：运行中删容器会让它拿着失效的句柄继续跑 -----------------
if pgrep -qf "$APP_NAME.app/Contents/MacOS/monosub"; then
  say "退出 ${APP_NAME}…"
  [ "$DRY_RUN" -eq 1 ] || osascript -e "quit app \"$APP_NAME\"" >/dev/null 2>&1 || true
  if [ "$DRY_RUN" -eq 0 ]; then
    for _ in 1 2 3 4 5; do
      pgrep -qf "$APP_NAME.app/Contents/MacOS/monosub" || break
      sleep 1
    done
    if pgrep -qf "$APP_NAME.app/Contents/MacOS/monosub"; then
      warn "优雅退出失败，强制结束"
      pkill -f "$APP_NAME.app/Contents/MacOS/monosub" || true
      sleep 1
    fi
  fi
else
  say "$APP_NAME 未在运行"
fi

# --- 2. 备份 settings.json（放到容器外，--full 之后还能找回） ----------------
BACKUP="/tmp/monosub-settings-$(date +%Y%m%d-%H%M%S).json"
if [ -f "$SETTINGS" ]; then
  say "备份设置：$BACKUP"
  [ "$DRY_RUN" -eq 1 ] || cp "$SETTINGS" "$BACKUP"
fi

# --- 3. 按模式清理 ----------------------------------------------------------
if [ "$MODE" = "grants" ]; then
  if [ -f "$SETTINGS" ]; then
    say "清除 settings.json 里的 dirGrants（模型目录/识别方案等设置保留）"
    if [ "$DRY_RUN" -eq 0 ]; then
      python3 - "$SETTINGS" <<'PY'
import json, sys
path = sys.argv[1]
with open(path, encoding='utf-8') as fh:
    data = json.load(fh)
removed = data.pop('dirGrants', None)
with open(path, 'w', encoding='utf-8') as fh:
    json.dump(data, fh, ensure_ascii=False, indent=2)
    fh.write('\n')
print(f"  移除 {len(removed or [])} 条授权")
PY
    fi
  else
    say "没有 settings.json，无需清理"
  fi

else
  # --- 全量：删整个容器 ---------------------------------------------------
  echo
  warn "以下内容将被删除（不可恢复）："
  [ -d "$CONTAINER" ] && du -sh "$CONTAINER" | sed 's/^/    /' || echo "    （容器不存在）"
  echo "    $CONTAINER"
  echo "    其中包含：设置、日志、缓存，以及全部 security-scoped bookmark"
  echo

  if [ "$ASSUME_YES" -ne 1 ] && [ -t 0 ]; then
    read -r -p "确认删除？（输入 yes 继续） " reply
    [ "$reply" = "yes" ] || die "已取消"
  elif [ "$ASSUME_YES" -ne 1 ]; then
    die "非交互模式下删除容器需要显式加 --yes"
  fi

  if [ "$DRY_RUN" -eq 0 ]; then
    rm -rf "$CONTAINER"
    say "容器已删除"
  else
    say "[dry-run] 将删除 $CONTAINER"
  fi
fi

# --- 4. TCC（通知等隐私项）。目录授权不走这里，清了也不影响 bookmark ---------
say "重置 TCC 记录（通知/麦克风等；目录授权不受影响）"
[ "$DRY_RUN" -eq 1 ] || tccutil reset All "$BUNDLE_ID" 2>/dev/null || warn "tccutil 无记录可清（正常）"

# --- 5. 结果 ---------------------------------------------------------------
echo
say "当前状态："
if [ -d "$CONTAINER" ]; then
  if [ -f "$SETTINGS" ]; then
    python3 - "$SETTINGS" <<'PY'
import json, sys
with open(sys.argv[1], encoding='utf-8') as fh:
    data = json.load(fh)
grants = data.get('dirGrants') or []
print(f"    容器：存在")
print(f"    modelsDir：{data.get('modelsDir') or '（默认：容器内 models/）'}")
print(f"    dirGrants：{len(grants)} 条")
for grant in grants:
    print(f"      - {grant['path']}")
PY
  else
    echo "    容器：存在但还没有 settings.json"
  fi
else
  echo "    容器：不存在（下次启动等于全新安装）"
fi

echo
cat <<'TIP'
下一步怎么验证：
  1. open src-tauri/target/mas/MonoSub.app
  2. 导入一个源目录未授权过的文件，点开始 → 应弹出目录面板
  3. 选好目录 → 字幕写进去
  4. 退出 app 再启动，重复第 2 步 → 不应再弹面板
     （日志里应有 "restored access to a user-granted folder"）
  5. 想让它"再问一次"：再跑一遍本脚本（默认 --grants）
TIP
