# Git Post-Commit Hook - Windows PowerShell 版本
# 模拟 git-ai 的 post-commit 功能
# 安装方法: 
#   1. 将此文件复制到 .git\hooks\
#   2. 重命名为 post-commit (无扩展名)
#   3. 在 .git\hooks\post-commit 中添加: #!/usr/bin/env pwsh 或 #!/usr/bin/env powershell

$ErrorActionPreference = "Stop"

# 配置
$GITAI_DIR = ".git\git-ai"
$NEW_COMMIT = (git rev-parse HEAD)
$BASE_COMMIT = (git rev-parse HEAD~1 2>$null)
if (-not $BASE_COMMIT) { $BASE_COMMIT = "initial" }

$WORKING_LOG_DIR = "$GITAI_DIR\working-logs\$BASE_COMMIT"
$CHECKPOINT_FILE = "$WORKING_LOG_DIR\checkpoints.json"
$TIMESTAMP = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()

Write-Host "📝 [Post-Commit] 处理提交归属..." -ForegroundColor Cyan

# 检查工作日志是否存在
if (-not (Test-Path $CHECKPOINT_FILE)) {
    Write-Host "⚠️  未找到工作日志，跳过" -ForegroundColor Yellow
    exit 0
}

# 获取实际提交的文件
$committedFiles = git diff-tree --no-commit-id --name-only -r $NEW_COMMIT

# 获取作者信息
$authorName = (git config user.name)
if (-not $authorName) { $authorName = "Unknown" }

$authorEmail = (git config user.email)
if (-not $authorEmail) { $authorEmail = "unknown@example.com" }

# 构建归属记录
$attestations = @()

foreach ($file in $committedFiles) {
    if ($file -and $file.Trim()) {
        $attestation = @{
            file         = $file -replace '\\', '/'  # 转换为Unix路径格式
            attributions = @(
                @{
                    start_line = 1
                    end_line   = 999999
                    author_id  = "Human"
                    timestamp  = $TIMESTAMP
                }
            )
        }
        
        $attestations += $attestation
    }
}

# 创建归属日志
$authorshipLog = @{
    version      = "1.0"
    metadata     = @{
        base_commit_sha = $NEW_COMMIT
        timestamp       = $TIMESTAMP
        author          = "$authorName <$authorEmail>"
        prompts         = @{}
    }
    attestations = $attestations
}

# 转换为JSON
$authorshipJson = $authorshipLog | ConvertTo-Json -Depth 10 -Compress

# 将归属日志附加到git notes
# PowerShell中需要使用临时文件
$tempFile = [System.IO.Path]::GetTempFileName()
$authorshipJson | Out-File -FilePath $tempFile -Encoding UTF8 -NoNewline

try {
    git notes --ref=git-ai add -f -F $tempFile $NEW_COMMIT 2>&1 | Out-Null
    Write-Host "✓ 归属日志已附加到 commit $NEW_COMMIT" -ForegroundColor Green
}
catch {
    Write-Host "⚠️  警告: 无法附加git notes: $_" -ForegroundColor Yellow
}
finally {
    Remove-Item -Path $tempFile -Force -ErrorAction SilentlyContinue
}

# 创建新的工作日志目录
$NEW_WORKING_LOG_DIR = "$GITAI_DIR\working-logs\$NEW_COMMIT"
New-Item -ItemType Directory -Force -Path "$NEW_WORKING_LOG_DIR\blobs" | Out-Null

# 清理旧工作日志（可选）
# Remove-Item -Path $WORKING_LOG_DIR -Recurse -Force

# 显示统计
$fileCount = ($committedFiles | Where-Object { $_ }).Count
Write-Host "📊 提交了 $fileCount 个文件" -ForegroundColor Yellow

Write-Host ""
exit 0
