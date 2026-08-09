#!/usr/bin/env bash
# ==============================================================================
# fetch-crates.sh
# Muskitty Crates - 批量拉取 / 推送 / 存在性检查工具 (macOS / Linux)
#
# 目录结构 (脚本放在项目根目录):
#   ./fetch-crates.sh       <-- 本脚本
#   ./crates/
#     ├── muskitty-cascade/            (独立仓库)
#     ├── muskitty-cssom/              (独立仓库)
#     ├── muskitty-renderer/           (主仓库 member，未剥离)
#     ├── muskitty-layout/             (独立仓库)
#     ├── muskitty-css/                (独立仓库)
#     ├── muskitty-css-parser/         (独立仓库)
#     ├── muskitty-css-tokenizer/      (独立仓库)
#     ├── muskitty-css-values/        (独立仓库)
#     ├── muskitty-dom/               (独立仓库)
#     ├── muskitty-html5-parser/      (独立仓库)
#     ├── muskitty-html5-tokenizer/   (独立仓库)
#     └── muskitty-selectors/         (独立仓库)
#
# 用法:
#   ./fetch-crates.sh                   # 默认: pull 模式 (拉取所有更新)
#   ./fetch-crates.sh pull              # 拉取所有独立仓库最新代码
#   ./fetch-crates.sh push              # 推送所有有本地提交的仓库
#   ./fetch-crates.sh status            # 仅检查状态，不拉取也不推送
#   ./fetch-crates.sh clone             # 首次克隆所有独立仓库
#   ./fetch-crates.sh -p ssh pull       # 使用 SSH 协议
#   ./fetch-crates.sh -p ssh push       # SSH 协议推送
#
# ==============================================================================

set -u  # 遇到未定义变量报错 (不用 set -e，单个失败不中断)

# ------------------------------------------------------------------------------
# 颜色定义
# ------------------------------------------------------------------------------
if [ -t 1 ]; then  # 仅在终端中启用颜色
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    MAGENTA='\033[0;35m'
    GRAY='\033[0;90m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    RED='' GREEN='' YELLOW='' BLUE='' CYAN='' MAGENTA='' GRAY='' BOLD='' NC=''
fi

# ------------------------------------------------------------------------------
# 配置
# ------------------------------------------------------------------------------
ORG="muskitty-dev"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATES_DIR="${SCRIPT_DIR}/crates"
PROTOCOL="https"

# 已独立拆分的 crate（需要单独拉取/推送）
STANDALONE_CRATES=(
    "muskitty-cascade"
    "muskitty-cssom"
    "muskitty-layout"
    "muskitty-css"
    "muskitty-css-parser"
    "muskitty-css-tokenizer"
    "muskitty-css-values"
    "muskitty-dom"
    "muskitty-html5-parser"
    "muskitty-html5-tokenizer"
    "muskitty-selectors"
)

# 尚未独立（作为主仓库 workspace member 直接版本控制，跳过）
BUNDLED_CRATES=(
    "muskitty-renderer"
)

# 汇总统计
TOTAL=0
SUCCESS=0
FAILED=0
SKIPPED=0
WARNINGS=0
declare -a FAILED_LIST=()
declare -a WARN_LIST=()

# ------------------------------------------------------------------------------
# 参数解析
# ------------------------------------------------------------------------------
MODE="pull"  # 默认模式

while [[ $# -gt 0 ]]; do
    case "$1" in
        -p|--protocol)
            PROTOCOL="$2"
            shift 2
            ;;
        -o|--org)
            ORG="$2"
            shift 2
            ;;
        -d|--dir)
            CRATES_DIR="$2"
            shift 2
            ;;
        pull|fetch)
            MODE="pull"
            shift
            ;;
        push)
            MODE="push"
            shift
            ;;
        status|check)
            MODE="status"
            shift
            ;;
        clone|init)
            MODE="clone"
            shift
            ;;
        -h|--help)
            sed -n '2,40p' "$0" | grep -v '^#!' | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo -e "${RED}未知参数: $1${NC}"
            echo "使用 -h 查看帮助"
            exit 1
            ;;
    esac
done

# ------------------------------------------------------------------------------
# 辅助函数
# ------------------------------------------------------------------------------

get_repo_url() {
    local repo="$1"
    if [[ "$PROTOCOL" == "ssh" ]]; then
        echo "git@github.com:${ORG}/${repo}.git"
    else
        echo "https://github.com/${ORG}/${repo}.git"
    fi
}

# 打印带颜色的状态行
# Usage: print_status LEVEL "message"
print_status() {
    local level="$1"
    local msg="$2"
    case "$level" in
        OK)      echo -e "  ${GREEN}[✓]${NC} $msg" ;;
        WARN)    echo -e "  ${YELLOW}[⚠]${NC} $msg" ;;
        ERROR)   echo -e "  ${RED}[✗]${NC} $msg" ;;
        INFO)    echo -e "  ${CYAN}[i]${NC} $msg" ;;
        SKIP)    echo -e "  ${GRAY}[-]${NC} $msg" ;;
        *)       echo "  $msg" ;;
    esac
}

# 检查远程仓库是否存在 (用 git ls-remote，不需要 API token)
# Usage: remote_exists "url"
remote_exists() {
    local url="$1"
    git ls-remote --exit-code --heads "$url" >/dev/null 2>&1
    return $?
}

# 检查是否是 git 仓库
is_git_repo() {
    local dir="$1"
    [[ -d "$dir/.git" ]] || [[ -f "$dir/.git" ]]
}

# 检查 git 仓库是否为空（无 commit）
is_empty_repo() {
    local dir="$1"
    local count
    count=$(git -C "$dir" rev-list --all --count 2>/dev/null || echo "0")
    [[ "$count" == "0" ]]
}

# 检查是否有未提交的更改
has_uncommitted() {
    local dir="$1"
    [[ -n "$(git -C "$dir" status --porcelain 2>/dev/null)" ]]
}

# 获取落后远程的 commit 数
get_behind_count() {
    local dir="$1"
    git -C "$dir" fetch origin --quiet 2>/dev/null || true
    local behind
    behind=$(git -C "$dir" rev-list --count HEAD..origin/HEAD 2>/dev/null || echo "0")
    echo "${behind:-0}"
}

# 获取当前分支名
get_branch() {
    local dir="$1"
    git -C "$dir" symbolic-ref --short HEAD 2>/dev/null || echo "(detached)"
}

# 分隔线
separator() {
    local name="$1"
    local rest=$(( 52 - ${#name} ))
    [[ $rest -lt 0 ]] && rest=0
    printf "${BLUE}━━━ %s ${GRAY}%${rest}s${NC}\n" "$name" "$(printf '─%.0s' $(seq 1 $rest))"
}

# ------------------------------------------------------------------------------
# 前置检查
# ------------------------------------------------------------------------------
clear
echo -e "${MAGENTA}${BOLD}"
echo "============================================================"
echo "  Muskitty Crates - 批量管理工具 (macOS/Linux)"
echo "============================================================"
echo -e "${NC}"
echo -e "  ${GRAY}组织:${NC}     $ORG"
echo -e "  ${GRAY}模式:${NC}     $MODE"
echo -e "  ${GRAY}协议:${NC}     $PROTOCOL"
echo -e "  ${GRAY}目录:${NC}     $CRATES_DIR"
echo -e "  ${GRAY}独立:${NC}     ${#STANDALONE_CRATES[@]} 个 crate"
echo -e "  ${GRAY}未独立:${NC}   ${#BUNDLED_CRATES[@]} 个 crate"
echo ""

# 检查 git
if ! command -v git >/dev/null 2>&1; then
    echo -e "${RED}[✗] 致命错误: 未找到 git 命令，请先安装${NC}"
    echo -e "  macOS:   brew install git"
    echo -e "  Ubuntu:  sudo apt install git"
    exit 1
fi

# 检查 curl (用于 GitHub API)
if ! command -v curl >/dev/null 2>&1; then
    echo -e "${YELLOW}[⚠] 警告: 未找到 curl，远程检查将使用 git ls-remote${NC}"
fi

# 确保 crates 目录存在
if [[ ! -d "$CRATES_DIR" ]]; then
    echo -e "${CYAN}[i] 创建 crates 目录: $CRATES_DIR${NC}"
    mkdir -p "$CRATES_DIR"
fi

# ------------------------------------------------------------------------------
# 核心处理函数
# ------------------------------------------------------------------------------

# 对单个 crate 执行 pull
do_pull() {
    local crate="$1"
    local repo_url
    repo_url=$(get_repo_url "$crate")
    local local_dir="${CRATES_DIR}/${crate}"

    separator "$crate"

    # 1. 检查远程是否存在
    if ! remote_exists "$repo_url"; then
        print_status ERROR "远程仓库不存在或不可访问"
        print_status ERROR "  $repo_url"
        FAILED=$((FAILED + 1))
        FAILED_LIST+=("$crate (远程不可访问)")
        return 1
    fi
    print_status OK "远程仓库存在"

    # 2. 本地不存在 → 克隆
    if [[ ! -d "$local_dir" ]]; then
        print_status INFO "本地不存在，正在克隆..."
        if git clone "$repo_url" "$local_dir" --quiet 2>&1 | while read -r line; do echo "    $line"; done; then
            print_status OK "克隆成功"
            SUCCESS=$((SUCCESS + 1))
        else
            print_status ERROR "克隆失败"
            FAILED=$((FAILED + 1))
            FAILED_LIST+=("$crate (克隆失败)")
        fi
        return
    fi

    # 3. 本地存在但不是 git 仓库
    if ! is_git_repo "$local_dir"; then
        if [[ -z "$(ls -A "$local_dir" 2>/dev/null)" ]]; then
            print_status WARN "空目录，正在克隆覆盖..."
            if git clone "$repo_url" "$local_dir" --quiet 2>&1; then
                print_status OK "克隆成功"
                SUCCESS=$((SUCCESS + 1))
            else
                print_status ERROR "克隆失败"
                FAILED=$((FAILED + 1))
                FAILED_LIST+=("$crate (克隆失败)")
            fi
        else
            print_status ERROR "目录非空且不是 Git 仓库，请手动处理"
            FAILED=$((FAILED + 1))
            FAILED_LIST+=("$crate (目录冲突)")
        fi
        return
    fi

    # 4. 是 git 仓库 → pull
    local branch
    branch=$(get_branch "$local_dir")

    if is_empty_repo "$local_dir"; then
        print_status WARN "空仓库（无 commit），尝试初始化..."
        # 空仓库：尝试从远程 checkout 默认分支
        local remote_head
        remote_head=$(git -C "$local_dir" rev-parse --abbrev-ref origin/HEAD 2>/dev/null || echo "")
        if [[ -n "$remote_head" ]]; then
            local branch_name="${remote_head#origin/}"
            git -C "$local_dir" checkout -b "$branch_name" "origin/$branch_name" --quiet 2>&1 && \
                print_status OK "已 checkout $branch_name" || \
                print_status ERROR "checkout 失败"
        else
            print_status WARN "远程无可用分支引用"
        fi
        # 空仓库也算处理过
        WARNINGS=$((WARNINGS + 1))
        WARN_LIST+=("$crate (空仓库)")
        return
    fi

    # 检查是否有未提交更改
    if has_uncommitted "$local_dir"; then
        print_status WARN "有未提交更改 (branch: $branch)"
        # 尝试 stash + pull + stash pop
        git -C "$local_dir" stash --quiet 2>/dev/null || true
        if git -C "$local_dir" pull --ff-only --quiet 2>&1; then
            git -C "$local_dir" stash pop --quiet 2>/dev/null || true
            print_status OK "已 stash/pull/pop 更新 (branch: $branch)"
            SUCCESS=$((SUCCESS + 1))
        else
            print_status ERROR "pull 失败，可能需要手动解决"
            FAILED=$((FAILED + 1))
            FAILED_LIST+=("$crate (pull 冲突)")
        fi
        return
    fi

    # 干净工作区：直接 pull
    local output
    if output=$(git -C "$local_dir" pull --ff-only --quiet 2>&1); then
        # 检查是否真的有更新
        if [[ "$output" == *"Already up to date"* ]] || [[ -z "$output" ]]; then
            print_status OK "已是最新 (branch: $branch)"
        else
            print_status OK "已更新 (branch: $branch)"
            echo "    $output" | head -3
        fi
        SUCCESS=$((SUCCESS + 1))
    else
        print_status ERROR "pull 失败: $output"
        FAILED=$((FAILED + 1))
        FAILED_LIST+=("$crate (pull 失败)")
    fi
}

# 对单个 crate 执行 push
do_push() {
    local crate="$1"
    local local_dir="${CRATES_DIR}/${crate}"

    separator "$crate"

    # 1. 检查本地存在且是 git 仓库
    if [[ ! -d "$local_dir" ]]; then
        print_status SKIP "本地不存在，跳过"
        SKIPPED=$((SKIPPED + 1))
        return
    fi

    if ! is_git_repo "$local_dir"; then
        print_status ERROR "不是 Git 仓库"
        FAILED=$((FAILED + 1))
        FAILED_LIST+=("$crate (非 Git 仓库)")
        return
    fi

    local branch
    branch=$(get_branch "$local_dir")

    # 2. 检查是否有需要推送的 commit
    local ahead
    ahead=$(git -C "$local_dir" rev-list --count origin/HEAD..HEAD 2>/dev/null || echo "0")

    if [[ "$ahead" == "0" ]]; then
        if has_uncommitted "$local_dir"; then
            print_status WARN "有未提交更改但未 commit (branch: $branch)"
            WARNINGS=$((WARNINGS + 1))
            WARN_LIST+=("$crate (有未提交更改)")
        else
            print_status SKIP "无待推送内容 (branch: $branch)"
            SKIPPED=$((SKIPPED + 1))
        fi
        return
    fi

    # 3. 检查未提交更改
    if has_uncommitted "$local_dir"; then
        print_status WARN "有未提交更改，仅推送已 commit 的内容 (branch: $branch)"
        WARNINGS=$((WARNINGS + 1))
    fi

    # 4. 推送
    print_status INFO "推送 $ahead 个 commit (branch: $branch)..."
    local output
    if output=$(git -C "$local_dir" push --quiet 2>&1); then
        print_status OK "推送成功 (branch: $branch)"
        SUCCESS=$((SUCCESS + 1))
    else
        print_status ERROR "推送失败: $output"
        FAILED=$((FAILED + 1))
        FAILED_LIST+=("$crate (push 失败)")
    fi
}

# 对单个 crate 执行 status 检查
do_status() {
    local crate="$1"
    local repo_url
    repo_url=$(get_repo_url "$crate")
    local local_dir="${CRATES_DIR}/${crate}"

    separator "$crate"

    # 远程检查
    if remote_exists "$repo_url"; then
        print_status OK "远程仓库存在"
    else
        print_status ERROR "远程仓库不可访问"
        FAILED=$((FAILED + 1))
        FAILED_LIST+=("$crate (远程不可访问)")
        return
    fi

    # 本地检查
    if [[ ! -d "$local_dir" ]]; then
        print_status WARN "本地不存在"
        WARNINGS=$((WARNINGS + 1))
        WARN_LIST+=("$crate (本地缺失)")
        return
    fi

    if ! is_git_repo "$local_dir"; then
        print_status ERROR "不是 Git 仓库"
        FAILED=$((FAILED + 1))
        FAILED_LIST+=("$crate (非 Git 仓库)")
        return
    fi

    local branch
    branch=$(get_branch "$local_dir")

    if is_empty_repo "$local_dir"; then
        print_status WARN "空仓库（无 commit）(branch: $branch)"
        WARNINGS=$((WARNINGS + 1))
        return
    fi

    # 获取落后/超前数
    git -C "$local_dir" fetch origin --quiet 2>/dev/null || true
    local behind
    behind=$(git -C "$local_dir" rev-list --count HEAD..origin/HEAD 2>/dev/null || echo "0")
    local ahead
    ahead=$(git -C "$local_dir" rev-list --count origin/HEAD..HEAD 2>/dev/null || echo "0")

    local status_parts=()
    [[ "$ahead" != "0" ]] && status_parts+=("领先 ${ahead}")
    [[ "$behind" != "0" ]] && status_parts+=("落后 ${behind}")

    if has_uncommitted "$local_dir"; then
        print_status WARN "有未提交更改 (branch: $branch)"
        WARNINGS=$((WARNINGS + 1))
        WARN_LIST+=("$crate (有未提交更改)")
    elif [[ "$ahead" != "0" && "$behind" != "0" ]]; then
        print_status WARN "分叉: $(IFS=', '; echo "${status_parts[*]}") (branch: $branch)"
        WARNINGS=$((WARNINGS + 1))
    elif [[ "$behind" != "0" ]]; then
        print_status WARN "落后远程 ${behind} 个提交 (branch: $branch)"
        WARNINGS=$((WARNINGS + 1))
        WARN_LIST+=("$crate (落后 ${behind})")
    elif [[ "$ahead" != "0" ]]; then
        print_status INFO "有 ${ahead} 个未推送提交 (branch: $branch)"
    else
        print_status OK "已同步 (branch: $branch)"
        SUCCESS=$((SUCCESS + 1))
    fi
}

# 对单个 crate 执行 clone
do_clone() {
    local crate="$1"
    local repo_url
    repo_url=$(get_repo_url "$crate")
    local local_dir="${CRATES_DIR}/${crate}"

    separator "$crate"

    if [[ -d "$local_dir" ]] && is_git_repo "$local_dir"; then
        print_status SKIP "已存在，跳过 (使用 pull 更新)"
        SKIPPED=$((SKIPPED + 1))
        return
    fi

    if [[ -d "$local_dir" ]] && ! is_git_repo "$local_dir"; then
        if [[ -z "$(ls -A "$local_dir" 2>/dev/null)" ]]; then
            print_status WARN "空目录，正在克隆..."
        else
            print_status ERROR "目录非空且不是 Git 仓库"
            FAILED=$((FAILED + 1))
            FAILED_LIST+=("$crate (目录冲突)")
            return
        fi
    fi

    print_status INFO "克隆中..."
    if git clone "$repo_url" "$local_dir" --quiet 2>&1 | while read -r line; do echo "    $line"; done; then
        print_status OK "克隆成功"
        SUCCESS=$((SUCCESS + 1))
    else
        print_status ERROR "克隆失败"
        FAILED=$((FAILED + 1))
        FAILED_LIST+=("$crate (克隆失败)")
    fi
}

# ------------------------------------------------------------------------------
# 主循环
# ------------------------------------------------------------------------------

TOTAL=${#STANDALONE_CRATES[@]}

case "$MODE" in
    pull)
        echo -e "${BOLD}模式: PULL - 拉取所有独立仓库更新${NC}\n"
        for crate in "${STANDALONE_CRATES[@]}"; do
            do_pull "$crate"
            echo ""
        done
        ;;
    push)
        echo -e "${BOLD}模式: PUSH - 推送所有有本地提交的仓库${NC}\n"
        for crate in "${STANDALONE_CRATES[@]}"; do
            do_push "$crate"
            echo ""
        done
        ;;
    status)
        echo -e "${BOLD}模式: STATUS - 检查所有仓库状态${NC}\n"
        for crate in "${STANDALONE_CRATES[@]}"; do
            do_status "$crate"
            echo ""
        done
        ;;
    clone)
        echo -e "${BOLD}模式: CLONE - 首次克隆所有独立仓库${NC}\n"
        for crate in "${STANDALONE_CRATES[@]}"; do
            do_clone "$crate"
            echo ""
        done
        ;;
esac

# ------------------------------------------------------------------------------
# 打印尚未独立的 crate 信息
# ------------------------------------------------------------------------------
echo ""
echo -e "${YELLOW}── 尚未独立（已包含在主仓库中，跳过）──${NC}"
for crate in "${BUNDLED_CRATES[@]}"; do
    local_dir="${CRATES_DIR}/${crate}"
    if [[ -d "$local_dir" ]]; then
        echo -e "  📦 ${crate}  ${GREEN}(本地 ✓)${NC}"
    else
        echo -e "  📦 ${crate}  ${GRAY}(本地 ✗)${NC}"
    fi
done

# ------------------------------------------------------------------------------
# 汇总报告
# ------------------------------------------------------------------------------
echo ""
echo -e "${MAGENTA}${BOLD}"
echo "============================================================"
echo "  汇总报告"
echo "============================================================"
echo -e "${NC}"

echo -e "${GRAY}模式: ${MODE} | 总计: ${TOTAL} 个独立仓库${NC}\n"

if [[ "$MODE" == "push" ]]; then
    echo -e "  ${GREEN}✓ 推送成功:  ${SUCCESS}${NC}"
    echo -e "  ${GRAY}- 无需推送:   ${SKIPPED}${NC}"
else
    echo -e "  ${GREEN}✓ 成功:       ${SUCCESS}${NC}"
fi

if [[ $WARNINGS -gt 0 ]]; then
    echo -e "  ${YELLOW}⚠ 警告:       ${WARNINGS}${NC}"
fi
if [[ $FAILED -gt 0 ]]; then
    echo -e "  ${RED}✗ 失败:       ${FAILED}${NC}"
fi

# 详细信息
if [[ ${#WARN_LIST[@]} -gt 0 ]]; then
    echo ""
    echo -e "${YELLOW}── 警告详情 ──${NC}"
    for item in "${WARN_LIST[@]}"; do
        echo -e "  ${YELLOW}⚠${NC} $item"
    done
fi

if [[ ${#FAILED_LIST[@]} -gt 0 ]]; then
    echo ""
    echo -e "${RED}── 失败详情 ──${NC}"
    for item in "${FAILED_LIST[@]}"; do
        echo -e "  ${RED}✗${NC} $item"
    done
fi

echo ""
echo "────────────────────────────────────────────────────────────"

if [[ $FAILED -gt 0 ]]; then
    echo -e "${RED}存在错误，退出码: 1${NC}"
    exit 1
else
    echo -e "${GREEN}全部正常 ✓${NC}"
    exit 0
fi
