#!/bin/bash
# Git-AI 核心 Git 命令参考
# 展示 git-ai 在 commit 前后使用的关键 Git 命令

echo "========================================="
echo "Git-AI 使用的核心 Git 命令"
echo "========================================="
echo ""

# ============================================
# PRE-COMMIT 阶段
# ============================================

echo "【PRE-COMMIT 阶段】"
echo ""

echo "1. 获取当前 HEAD commit SHA"
echo "   git rev-parse HEAD"
echo ""

echo "2. 获取所有文件变更状态（包括已暂存和未暂存）"
echo "   git status --porcelain=v2"
echo ""

echo "3. 对每个变更文件执行 diff 分析"
echo "   git diff HEAD -- <file>"
echo ""

echo "4. 使用 git blame 追溯代码作者"
echo "   git blame --line-porcelain <file>"
echo ""

echo "5. 计算文件内容哈希"
echo "   sha256sum <file>"
echo "   保存位置: .git/git-ai/working-logs/<commit-sha>/blobs/"
echo ""

# ============================================
# POST-COMMIT 阶段
# ============================================

echo "【POST-COMMIT 阶段】"
echo ""

echo "6. 获取新创建的 commit SHA"
echo "   git rev-parse HEAD"
echo ""

echo "7. 获取此次提交实际包含的文件"
echo "   git diff-tree --no-commit-id --name-only -r <commit-sha>"
echo ""

echo "8. 读取文件在 HEAD 中的内容"
echo "   git show HEAD:<file>"
echo ""

echo "9. 将归属日志附加到 commit 的 git notes"
echo "   git notes --ref=git-ai add -f -m '<json-data>' <commit-sha>"
echo ""

echo "10. 查看某个 commit 的 git-ai notes"
echo "    git notes --ref=git-ai show <commit-sha>"
echo ""

# ============================================
# 查询命令
# ============================================

echo "【查询命令】"
echo ""

echo "11. 查看文件的 AI 代码分布"
echo "    git-ai blame <file>"
echo "    等价实现:"
echo "      - git blame <file>"
echo "      - git notes --ref=git-ai show <commit>"
echo "      - 合并数据并高亮 AI 行"
echo ""

echo "12. 查看某次提交的 AI 统计"
echo "    git-ai show <commit>"
echo "    等价于:"
echo "      git notes --ref=git-ai show <commit>"
echo ""

echo "13. 查看提交历史的 git-ai notes"
echo "    git log --show-notes=git-ai"
echo ""

# ============================================
# 数据结构
# ============================================

echo "【数据结构】"
echo ""

echo "工作日志存储位置:"
echo ".git/git-ai/working-logs/<base-commit-sha>/"
echo "  ├── checkpoints.json    # 检查点列表"
echo "  ├── blobs/              # 文件快照"
echo "  └── INITIAL             # 初始归属"
echo ""

echo "归属日志存储:"
echo "git notes --ref=git-ai (附加到每个 commit)"
echo ""

echo "========================================="
echo "完成"
echo "========================================="
