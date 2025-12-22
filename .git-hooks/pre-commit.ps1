# Git Pre-Commit Hook - Windows PowerShell 版本
# 模拟 git-ai 的 pre-commit 功能
# 安装方法: 
#   1. 将此文件复制到 .git\hooks\
#   2. 重命名为 pre-commit (无扩展名)
#   3. 在 .git\hooks\pre-commit 中添加: #!/usr/bin/env pwsh 或 #!/usr/bin/env powershell

$ErrorActionPreference = "Stop"

# 配置
$GITAI_DIR = ".git\git-ai"
$BASE_COMMIT = (git rev-parse HEAD 2>$null)
if (-not $BASE_COMMIT) { $BASE_COMMIT = "initial" }

$WORKING_LOG_DIR = "$GITAI_DIR\working-logs\$BASE_COMMIT"
$BLOBS_DIR = "$WORKING_LOG_DIR\blobs"
$CHECKPOINT_FILE = "$WORKING_LOG_DIR\checkpoints.json"
$TIMESTAMP = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()

# 创建必要目录
New-Item -ItemType Directory -Force -Path $BLOBS_DIR | Out-Null

Write-Host "🔍 [Pre-Commit] 扫描文件变更..." -ForegroundColor Cyan

# 获取变更的文本文件
function Get-ChangedTextFiles {
    $changedFiles = @()
    
    # 获取git status
    $statusOutput = git status --porcelain=v2
    
    foreach ($line in $statusOutput) {
        # 解析 porcelain v2 格式
        if ($line -match '^1 |^2 ') {
            # 提取文件路径（第9个字段开始）
            $parts = $line -split '\s+', 9
            if ($parts.Count -ge 9) {
                $filePath = $parts[8]
                
                # 检查文件是否存在且为文本文件
                if (Test-Path $filePath -PathType Leaf) {
                    # 简单检测：尝试读取为文本
                    try {
                        $content = Get-Content $filePath -Raw -ErrorAction Stop
                        # 检查是否包含null字节（二进制文件）
                        if ($content -notmatch [char]0) {
                            $changedFiles += $filePath
                        }
                    }
                    catch {
                        # 无法读取为文本，跳过
                    }
                }
            }
        }
    }
    
    return $changedFiles
}

# 计算文件的SHA256哈希
function Get-FileSHA256 {
    param([string]$FilePath)
    
    $hash = Get-FileHash -Path $FilePath -Algorithm SHA256
    return $hash.Hash.ToLower()
}

# 保存文件快照
function Save-FileSnapshots {
    param([string[]]$Files)
    
    $entries = @()
    
    foreach ($file in $Files) {
        if (Test-Path $file -PathType Leaf) {
            # 计算SHA256哈希
            $hash = Get-FileSHA256 -FilePath $file
            
            # 保存文件快照
            $blobPath = Join-Path $BLOBS_DIR $hash
            Copy-Item -Path $file -Destination $blobPath -Force
            
            # 创建entry对象
            $entry = @{
                file = $file -replace '\\', '/'  # 转换为Unix路径格式
                blob_sha = $hash
                attributions = @()
                line_attributions = @()
            }
            
            $entries += $entry
        }
    }
    
    return $entries
}

# 获取变更文件列表
$changedFiles = Get-ChangedTextFiles

if ($changedFiles.Count -eq 0) {
    Write-Host "✓ 无文本文件变更" -ForegroundColor Green
    exit 0
}

Write-Host "📝 发现 $($changedFiles.Count) 个变更文件" -ForegroundColor Yellow

# 保存文件快照
$entries = Save-FileSnapshots -Files $changedFiles

# 获取作者信息
$authorName = (git config user.name)
if (-not $authorName) { $authorName = "Unknown" }

$authorEmail = (git config user.email)
if (-not $authorEmail) { $authorEmail = "unknown@example.com" }

# 创建检查点JSON
$checkpoint = @{
    version = "1.0"
    checkpoints = @(
        @{
            kind = "Human"
            timestamp = $TIMESTAMP
            author = "$authorName <$authorEmail>"
            diff_hash = (Get-FileHash -InputStream ([System.IO.MemoryStream]::new([Text.Encoding]::UTF8.GetBytes(($changedFiles -join ',')))) -Algorithm SHA256).Hash.ToLower()
            entries = $entries
            line_stats = @{
                additions = 0
                deletions = 0
            }
        }
    )
}

# 保存检查点文件
$checkpointJson = $checkpoint | ConvertTo-Json -Depth 10
$checkpointJson | Out-File -FilePath $CHECKPOINT_FILE -Encoding UTF8 -Force

Write-Host "✓ 检查点已创建: $CHECKPOINT_FILE" -ForegroundColor Green
Write-Host ""

exit 0
