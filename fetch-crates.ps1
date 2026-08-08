#!/usr/bin/env pwsh
# ==============================================================================
# fetch-crates.ps1
# 放在项目根目录运行，自动拉取 muskitty-dev 下所有 crates 子仓库
# 目录结构:
#   ./fetch-crates.ps1       <-- 本脚本
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
#   pwsh ./fetch-crates.ps1                # 检查 + 克隆缺失的
#   pwsh ./fetch-crates.ps1 -Force         # 强制拉取更新所有
#   pwsh ./fetch-crates.ps1 -Protocol ssh  # 使用 SSH 协议
# ==============================================================================

[CmdletBinding()]
param(
    [Parameter(Mandatory = $false)]
    [ValidatePattern('^[A-Za-z0-9_.-]+$')]
    [string]$Org = 'muskitty-dev',

    [Parameter(Mandatory = $false)]
    [string]$CratesDir = (Join-Path $PSScriptRoot 'crates'),

    [Parameter(Mandatory = $false)]
    [switch]$Force,

    [Parameter(Mandatory = $false)]
    [ValidateSet('https', 'ssh')]
    [string]$Protocol = 'https'
)

$ErrorActionPreference = 'Continue'  # 单个失败不中断整体

# ------------------------------------------------------------------------------
# 已独立拆分的 crate 仓库（需要从 GitHub 单独拉取）
# ------------------------------------------------------------------------------
$StandaloneCrates = @(
    'muskitty-cascade',
    'muskitty-cssom',
    'muskitty-layout',
    'muskitty-css',
    'muskitty-css-parser',
    'muskitty-css-tokenizer',
    'muskitty-css-values',
    'muskitty-dom',
    'muskitty-html5-parser',
    'muskitty-html5-tokenizer',
    'muskitty-selectors'
)

# ------------------------------------------------------------------------------
# 尚未独立拆分的 crate（作为主仓库 workspace member 直接版本控制，跳过）
# ------------------------------------------------------------------------------
$BundledCrates = @(
    'muskitty-renderer'
)

# ------------------------------------------------------------------------------
# 辅助函数
# ------------------------------------------------------------------------------

function Get-RepoUrl {
    param([string]$Organization, [string]$RepoName, [string]$Proto)
    if ($Proto -eq 'ssh') {
        return "git@github.com:$Organization/$RepoName.git"
    }
    return "https://github.com/$Organization/$RepoName.git"
}

function Test-RemoteExists {
    <#
    .SYNOPSIS
    检查 GitHub 远程仓库是否存在（先 API，失败 fallback 到 git ls-remote）
    #>
    param([string]$Organization, [string]$RepoName, [string]$RepoUrl)

    # 方法1: GitHub API（无需认证，匿名 60次/小时够用）
    $apiUrl = "https://api.github.com/repos/$Organization/$RepoName"
    try {
        $resp = Invoke-RestMethod -Uri $apiUrl -Method Get -TimeoutSec 8 -ErrorAction Stop
        return $true
    }
    catch [System.Net.WebException] {
        $code = [int]$_.Exception.Response.StatusCode
        if ($code -eq 404) { return $false }
        # 其他错误（限流/网络），fallthrough 到方法2
    }
    catch { }

    # 方法2: git ls-remote fallback
    try {
        $null = git ls-remote --exit-code --heads $RepoUrl 2>$null
        return $true
    }
    catch { return $false }
}

function Test-IsEmptyGitRepo {
    <#
    .SYNOPSIS
    检查 Git 仓库是否没有任何提交（空仓库）
    #>
    param([string]$Path)
    try {
        $commitCount = git -C $Path rev-list --all --count 2>$null
        return ([int]$commitCount -eq 0)
    }
    catch { return $true }
}

function Test-HasUncommittedChanges {
    param([string]$Path)
    try {
        $status = git -C $Path status --porcelain 2>$null
        return (-not [string]::IsNullOrWhiteSpace($status))
    }
    catch { return $false }
}

function Get-BehindCount {
    param([string]$Path)
    try {
        # 确保有远程引用
        git -C $Path fetch origin --quiet 2>$null
        $behind = git -C $Path rev-list --count HEAD..origin/HEAD 2>$null
        if ([string]::IsNullOrEmpty($behind)) { $behind = '0' }
        return [int]$behind
    }
    catch { return 0 }
}

# ------------------------------------------------------------------------------
# 前置检查
# ------------------------------------------------------------------------------

Write-Host ""
Write-Host "============================================================" -ForegroundColor Magenta
Write-Host "  Muskitty Crates - 批量拉取 & 存在性检查工具" -ForegroundColor Magenta
Write-Host "============================================================" -ForegroundColor Magenta
Write-Host ""
Write-Host "  组织:     $Org" -ForegroundColor Gray
Write-Host "  Crates目录: $CratesDir" -ForegroundColor Gray
Write-Host "  协议:     $Protocol" -ForegroundColor Gray
Write-Host "  强制更新: $($Force.IsPresent)" -ForegroundColor Gray
Write-Host "  脚本位置: $PSScriptRoot" -ForegroundColor Gray
Write-Host ""

# 检查 git
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host "[✗] 致命错误: 未找到 git 命令，请先安装 Git for Windows" -ForegroundColor Red
    Write-Host "    下载: https://git-scm.com/download/win" -ForegroundColor Red
    exit 1
}

# 确保 crates 目录存在
if (-not (Test-Path -Path $CratesDir -PathType Container)) {
    Write-Host "[i] 创建 crates 目录: $CratesDir" -ForegroundColor Cyan
    New-Item -Path $CratesDir -ItemType Directory -Force | Out-Null
}

Write-Host ""
Write-Host "开始处理 $($StandaloneCrates.Count) 个独立 crate（另有 $($BundledCrates.Count) 个尚未独立，已跳过）..." -ForegroundColor White
Write-Host ""

# ------------------------------------------------------------------------------
# 主循环
# ------------------------------------------------------------------------------

$results = [System.Collections.Generic.List[object]]::new()

foreach ($crate in $StandaloneCrates) {
    Write-Host "━━━ $crate " -ForegroundColor Blue -NoNewline
    Write-Host "$('─' * (52 - $crate.Length))" -ForegroundColor DarkBlue

    $repoUrl  = Get-RepoUrl -Organization $Org -RepoName $crate -Proto $Protocol
    $localDir = Join-Path $CratesDir $crate

    $entry = [PSCustomObject]@{
        Crate       = $crate
        RemoteOK    = $false
        LocalExists = $false
        IsGitRepo   = $false
        IsEmpty     = $true
        HasChanges  = $false
        Behind      = 0
        Action      = ''
        Status      = ''
    }

    # ---- 1. 远程检查 ----
    $remoteOk = Test-RemoteExists -Organization $Org -RepoName $crate -RepoUrl $repoUrl
    $entry.RemoteOK = $remoteOk

    if (-not $remoteOk) {
        Write-Host "  [✗] 远程仓库不存在或不可访问" -ForegroundColor Red
        Write-Host "      期望: $repoUrl" -ForegroundColor DarkRed
        $entry.Action = 'Skip'
        $entry.Status = 'RemoteMissing'
        $results.Add($entry)
        Write-Host ""
        continue
    }

    Write-Host "  [✓] 远程仓库存在" -ForegroundColor Green

    # ---- 2. 本地检查 ----
    if (Test-Path -Path $localDir) {
        $entry.LocalExists = $true

        $gitDir = Join-Path $localDir '.git'
        if (Test-Path -Path $gitDir) {
            # 是 Git 仓库
            $entry.IsGitRepo = $true

            # 检查是否为空仓库（无 commit）
            $isEmpty = Test-IsEmptyGitRepo -Path $localDir
            $entry.IsEmpty = $isEmpty

            if ($isEmpty) {
                Write-Host "  [⚠] 本地是空 Git 仓库（无 commit）" -ForegroundColor Yellow
            }

            # 检查未提交更改
            $hasChanges = Test-HasUncommittedChanges -Path $localDir
            $entry.HasChanges = $hasChanges
            if ($hasChanges) {
                Write-Host "  [⚠] 存在未提交的本地更改" -ForegroundColor Yellow
            }

            # 检查落后远程多少
            if (-not $isEmpty) {
                $behind = Get-BehindCount -Path $localDir
                $entry.Behind = $behind
                if ($behind -gt 0) {
                    Write-Host "  [⚠] 本地落后远程 $behind 个提交" -ForegroundColor Yellow
                }
            }

            # ---- 3. 更新逻辑 ----
            if ($Force) {
                Write-Host "  [...] 正在更新 (Force)..." -ForegroundColor Cyan
                try {
                    # 先 fetch
                    git -C $localDir fetch origin --quiet 2>&1 | Out-Null

                    if ($isEmpty) {
                        # 空仓库：checkout 默认分支
                        $defaultBranch = git -C $localDir symbolic-ref --short HEAD 2>$null
                        if ([string]::IsNullOrEmpty($defaultBranch)) {
                            # 没有 HEAD，尝试从远程 checkout
                            $remoteHead = git -C $localDir rev-parse --abbrev-ref origin/HEAD 2>$null
                            if (-not [string]::IsNullOrEmpty($remoteHead)) {
                                $branchName = $remoteHead -replace 'origin/', ''
                                git -C $localDir checkout -b $branchName origin/$branchName 2>&1 | Out-Null
                            }
                        }
                        $entry.Action = 'Init'
                        $entry.Status = 'EmptyRepoInitialized'
                        Write-Host "  [✓] 空仓库已初始化" -ForegroundColor Green
                    }
                    elseif ($hasChanges) {
                        # 有未提交更改：stash 后 pull 再 pop
                        git -C $localDir stash --quiet 2>&1 | Out-Null
                        git -C $localDir pull --ff-only --quiet 2>&1 | Out-Null
                        git -C $localDir stash pop --quiet 2>&1 | Out-Null
                        $entry.Action = 'StashPullPop'
                        $entry.Status = 'UpdatedWithStash'
                        Write-Host "  [✓] 已 stash/pull/pop 更新" -ForegroundColor Green
                    }
                    else {
                        # 干净工作区：直接 pull
                        git -C $localDir pull --ff-only --quiet 2>&1 | Out-Null
                        if ($LASTEXITCODE -ne 0) {
                            # fast-forward 失败，硬重置
                            git -C $localDir reset --hard origin/HEAD --quiet 2>&1 | Out-Null
                            $entry.Action = 'Reset'
                            $entry.Status = 'ForceReset'
                            Write-Host "  [✓] 已强制重置到远程 HEAD" -ForegroundColor Green
                        }
                        else {
                            $entry.Action = 'Pull'
                            $entry.Status = 'Updated'
                            Write-Host "  [✓] 已更新到最新" -ForegroundColor Green
                        }
                    }
                }
                catch {
                    $entry.Action = 'Error'
                    $entry.Status = "UpdateFailed: $_"
                    Write-Host "  [✗] 更新失败: $_" -ForegroundColor Red
                }
            }
            else {
                # 不强制：仅报告状态
                if ($isEmpty) {
                    $entry.Action = 'None'
                    $entry.Status = 'EmptyRepo'
                    Write-Host "  [i] 空仓库，加 -Force 可初始化" -ForegroundColor Cyan
                }
                elseif ($hasChanges) {
                    $entry.Action = 'None'
                    $entry.Status = 'LocalChanges'
                    Write-Host "  [i] 有本地更改，加 -Force 可 stash 后更新" -ForegroundColor Cyan
                }
                elseif ($entry.Behind -gt 0) {
                    $entry.Action = 'None'
                    $entry.Status = "Behind($($entry.Behind))"
                    Write-Host "  [i] 落后远程，加 -Force 可更新" -ForegroundColor Cyan
                }
                else {
                    $entry.Action = 'None'
                    $entry.Status = 'UpToDate'
                    Write-Host "  [✓] 已是最新" -ForegroundColor Green
                }
            }
        }
        else {
            # 目录存在但不是 Git 仓库
            $childCount = (Get-ChildItem -Path $localDir -Force).Count
            if ($childCount -eq 0) {
                # 空目录 → 直接 clone
                Write-Host "  [...] 空目录，正在克隆..." -ForegroundColor Cyan
                try {
                    git clone $repoUrl $localDir --quiet 2>&1 | Out-Null
                    $entry.IsGitRepo = $true
                    $entry.IsEmpty = $false
                    $entry.Action = 'Clone'
                    $entry.Status = 'Cloned'
                    Write-Host "  [✓] 克隆成功" -ForegroundColor Green
                }
                catch {
                    $entry.Action = 'Error'
                    $entry.Status = "CloneFailed: $_"
                    Write-Host "  [✗] 克隆失败: $_" -ForegroundColor Red
                }
            }
            else {
                $entry.Action = 'Error'
                $entry.Status = 'DirNotEmpty_NotGit'
                Write-Host "  [✗] 目录非空且不是 Git 仓库，请手动处理" -ForegroundColor Red
            }
        }
    }
    else {
        # 本地不存在 → 克隆
        Write-Host "  [...] 正在克隆..." -ForegroundColor Cyan
        try {
            git clone $repoUrl $localDir --quiet 2>&1 | Out-Null
            $entry.LocalExists = $true
            $entry.IsGitRepo = $true
            $entry.IsEmpty = $false
            $entry.Action = 'Clone'
            $entry.Status = 'Cloned'
            Write-Host "  [✓] 克隆成功" -ForegroundColor Green
        }
        catch {
            $entry.Action = 'Error'
            $entry.Status = "CloneFailed: $_"
            Write-Host "  [✗] 克隆失败: $_" -ForegroundColor Red
        }
    }

    $results.Add($entry)
    Write-Host ""
}

# ------------------------------------------------------------------------------
# 将尚未独立的 crate 作为 Bundled 条目加入结果
# ------------------------------------------------------------------------------
foreach ($bundled in $BundledCrates) {
    $localDir = Join-Path $CratesDir $bundled
    $localExists = Test-Path -Path $localDir
    $isGit = $false
    if ($localExists) {
        $isGit = Test-Path -Path (Join-Path $localDir '.git')
    }

    $results.Add([PSCustomObject]@{
        Crate       = $bundled
        RemoteOK    = $false
        LocalExists = $localExists
        IsGitRepo   = $isGit
        IsEmpty     = $false
        HasChanges  = $false
        Behind      = 0
        Action      = 'Bundled'
        Status      = 'BundledInMain'
    })
}

# ------------------------------------------------------------------------------
# 汇总报告
# ------------------------------------------------------------------------------

Write-Host "============================================================" -ForegroundColor Magenta
Write-Host "  汇总报告" -ForegroundColor Magenta
Write-Host "============================================================" -ForegroundColor Magenta
Write-Host ""

# 表格
$results | ForEach-Object {
    $isBundled = ($_.Action -eq 'Bundled')
    [PSCustomObject]@{
        仓库       = if ($isBundled) { "$($_.Crate) 📦" } else { $_.Crate }
        远程       = if ($isBundled) { 'n/a' } elseif ($_.RemoteOK) { '✓' } else { '✗' }
        本地       = if ($_.LocalExists) { '✓' } else { '—' }
        Git仓库    = if ($_.IsGitRepo) { '✓' } else { '—' }
        非空       = if ($isBundled) { 'n/a' } elseif (-not $_.IsEmpty) { '✓' } else { '⚠空' }
        状态       = if ($isBundled) { 'BundledInMain(在主仓库内)' } else { $_.Status }
    }
} | Format-Table -AutoSize -Wrap | Out-String -Width 140 | Write-Host

# 统计
$standalone = $results | Where-Object { $_.Action -ne 'Bundled' }
$bundled    = $results | Where-Object { $_.Action -eq 'Bundled' }
$total       = $results.Count
$remoteMiss  = ($standalone | Where-Object { -not $_.RemoteOK }).Count
$cloned      = ($standalone | Where-Object { $_.Action -eq 'Clone' }).Count
$updated     = ($standalone | Where-Object { $_.Status -in @('Updated', 'UpdatedWithStash', 'ForceReset', 'EmptyRepoInitialized') }).Count
$upToDate    = ($standalone | Where-Object { $_.Status -eq 'UpToDate' }).Count
$emptyRepos  = ($standalone | Where-Object { $_.IsEmpty -and $_.IsGitRepo }).Count
$warnings    = ($standalone | Where-Object { $_.Status -match 'LocalChanges|Behind|EmptyRepo' }).Count
$errors      = ($standalone | Where-Object { $_.Status -match 'Failed|Missing|Conflict|NotEmpty' }).Count

Write-Host "────────────────────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host "总计: $total 个 crate（独立: $($standalone.Count)，未独立: $($bundled.Count)）" -ForegroundColor White
Write-Host ""
Write-Host "── 独立仓库 ──" -ForegroundColor Cyan
if ($cloned -gt 0)    { Write-Host "  [+] 新克隆:        $cloned 个" -ForegroundColor Green }
if ($updated -gt 0)   { Write-Host "  [~] 已更新:        $updated 个" -ForegroundColor Green }
if ($upToDate -gt 0)  { Write-Host "  [✓] 已是最新:      $upToDate 个" -ForegroundColor Green }
if ($emptyRepos -gt 0) { Write-Host "  [⚠] 空仓库:        $emptyRepos 个（无 commit）" -ForegroundColor Yellow }
if ($warnings -gt 0)  { Write-Host "  [i] 需注意:        $warnings 个（使用 -Force 可修复）" -ForegroundColor Cyan }
if ($remoteMiss -gt 0) { Write-Host "  [✗] 远程缺失:      $remoteMiss 个" -ForegroundColor Red }
if ($errors -gt 0)    { Write-Host "  [✗] 错误:          $errors 个" -ForegroundColor Red }

if ($bundled.Count -gt 0) {
    Write-Host ""
    Write-Host "── 尚未独立（已包含在主仓库中，跳过远程检查）──" -ForegroundColor DarkYellow
    $bundled | ForEach-Object {
        $mark = if ($_.LocalExists) { '✓' } else { '—' }
        Write-Host "  📦 $($_.Crate)  (本地: $mark)" -ForegroundColor DarkYellow
    }
}

Write-Host ""
Write-Host "────────────────────────────────────────────────────────────" -ForegroundColor DarkGray

if ($errors -gt 0 -or $remoteMiss -gt 0) {
    Write-Host "存在错误，退出码: 1" -ForegroundColor Red
    exit 1
}
else {
    Write-Host "全部正常 ✓" -ForegroundColor Green
    exit 0
}
