# git-ai-for-turingcoder

追踪图灵 coder 中的 AI 代码生成率情况。

## 使用说明 / Usage

### 基本工作流程 / Basic Workflow

```bash
# 人为写完代码后执行 / After human writes code
git-ai checkpoint

# AI写完代码后执行 / After AI writes code
git-ai checkpoint mock_ai

# 查看统计 / View statistics
git-ai working-stats

# 统计命令可以加--json参数 / Add --json parameter for JSON output
git-ai working-stats --json
```

## 安装 / Installation

### 从源码构建 / Build from Source

```bash
# 克隆仓库 / Clone repository
git clone https://github.com/ShuboLiang/git-ai-for-turingcoder.git
cd git-ai-for-turingcoder

# 构建项目 / Build project
cargo build --release

# 二进制文件位置 / Binary location
# target/release/git-ai.exe (Windows)
# target/release/git-ai (Linux/macOS)
```
