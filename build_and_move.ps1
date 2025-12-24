# build_and_move.ps1

# Step 1: 运行 cargo build --release
Write-Host "正在执行 cargo build --release ..." -ForegroundColor Cyan
cargo build --release

# 检查上一步是否成功
if ($LASTEXITCODE -ne 0) {
    Write-Host "构建失败，退出脚本。" -ForegroundColor Red
    exit $LASTEXITCODE
}

# Step 2: 定义路径
$sourcePath = "target\release\git-ai.exe"
$destinationDir = "C:\Users\liangshubo\Desktop\projects\demo"
$destinationPath = Join-Path $destinationDir "git-ai.exe"
$testBatPath = Join-Path $destinationDir "test.bat"

# Step 3: 确保目标目录存在
if (!(Test-Path $destinationDir)) {
    Write-Host "目标目录不存在，正在创建: $destinationDir" -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $destinationDir | Out-Null
}

# Step 4: 移动（覆盖）可执行文件
Write-Host "正在将 $sourcePath 移动到 $destinationPath ..." -ForegroundColor Cyan
Move-Item -Path $sourcePath -Destination $destinationPath -Force

# Step 5: 检查 test.bat 是否存在
if (!(Test-Path $testBatPath)) {
    Write-Host "警告：未找到 test.bat 脚本 ($testBatPath)，跳过执行。" -ForegroundColor Yellow
}
else {
    Write-Host "正在执行 $testBatPath ..." -ForegroundColor Cyan
    # 在目标目录中启动 test.bat（保持当前 PowerShell 窗口可见输出）
    Push-Location $destinationDir
    cmd /c "test.bat"
    Pop-Location
}

Write-Host "全部操作完成！" -ForegroundColor Green