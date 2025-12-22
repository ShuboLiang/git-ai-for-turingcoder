#!/bin/bash
# Git-AI 等价操作演示脚本
# 这个脚本演示了 git-ai 在 git commit 前后做的核心操作

set -e

# 颜色定义
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Git-AI Commit 前后操作演示${NC}"
echo -e "${BLUE}========================================${NC}\n"

# ============================================
# PRE-COMMIT: Git Commit 之前的操作
# ============================================

echo -e "${GREEN}[PRE-COMMIT] 开始执行 commit 前的操作...${NC}\n"

# 1. 获取当前 HEAD commit
echo -e "${YELLOW}步骤 1: 获取当前 HEAD commit${NC}"
BASE_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "initial")
echo "BASE_COMMIT = $BASE_COMMIT"
echo ""

# 2. 扫描文件变更（包括已暂存和未暂存的）
echo -e "${YELLOW}步骤 2: 扫描所有变更的文件${NC}"
echo "执行命令: git status --porcelain=v2"
git status --porcelain=v2 | head -20
echo ""

# 3. 获取变更文件列表（只处理文本文件）
echo -e "${YELLOW}步骤 3: 获取变更的文本文件列表${NC}"
CHANGED_FILES=$(git status --porcelain=v2 | \
    awk '$1 == "1" || $1 == "2" {
        # 提取文件路径（第9个字段开始）
        for(i=9; i<=NF; i++) printf "%s%s", $i, (i<NF?" ":"")
        print ""
    }' | \
    while read file; do
        # 简单判断是否为文本文件（实际git-ai会检查是否包含null字节）
        if [[ -f "$file" ]] && file "$file" | grep -q "text"; then
            echo "$file"
        fi
    done)

echo "变更的文本文件:"
echo "$CHANGED_FILES"
echo ""

# 4. 为每个文件创建快照并计算内容哈希
echo -e "${YELLOW}步骤 4: 为文件创建快照（保存到 .git/git-ai/blobs/）${NC}"
WORKING_LOG_DIR=".git/git-ai/working-logs/$BASE_COMMIT"
BLOBS_DIR="$WORKING_LOG_DIR/blobs"
mkdir -p "$BLOBS_DIR"

echo "模拟保存文件快照到: $BLOBS_DIR"
echo "$CHANGED_FILES" | while read file; do
    if [[ -n "$file" ]]; then
        # 计算文件内容的 SHA256 哈希
        CONTENT_HASH=$(sha256sum "$file" | cut -d' ' -f1)
        echo "  $file -> $CONTENT_HASH"
        
        # 保存文件快照（实际操作）
        cp "$file" "$BLOBS_DIR/$CONTENT_HASH" 2>/dev/null || true
    fi
done
echo ""

# 5. 读取之前的工作日志（如果存在）
echo -e "${YELLOW}步骤 5: 读取之前的工作日志${NC}"
if [[ -f "$WORKING_LOG_DIR/checkpoints.json" ]]; then
    echo "找到之前的工作日志: $WORKING_LOG_DIR/checkpoints.json"
    echo "前几行内容:"
    head -5 "$WORKING_LOG_DIR/checkpoints.json" || true
else
    echo "未找到之前的工作日志，这是首次检查点"
fi
echo ""

# 6. 分析代码归属（这是最核心的部分）
echo -e "${YELLOW}步骤 6: 分析代码归属（AI vs 人类）${NC}"
echo "对于每个文件执行以下操作:"
echo "  a) 使用 git diff 比对当前内容与上个版本"
echo "  b) 对于新增/修改的行，检查是否有 AI 检查点标记"
echo "  c) 使用 git blame 追溯未标记行的作者"
echo "  d) 创建行级归属记录"
echo ""

echo "模拟命令示例:"
if [[ -n "$CHANGED_FILES" ]]; then
    FIRST_FILE=$(echo "$CHANGED_FILES" | head -1)
    if [[ -n "$FIRST_FILE" ]] && [[ -f "$FIRST_FILE" ]]; then
        echo "# 查看第一个文件的 diff"
        echo "git diff HEAD -- \"$FIRST_FILE\" | head -20"
        git diff HEAD -- "$FIRST_FILE" 2>/dev/null | head -20 || echo "(文件可能是新增的)"
    fi
fi
echo ""

# 7. 创建检查点并保存到工作日志
echo -e "${YELLOW}步骤 7: 创建 Human 检查点并保存${NC}"
CHECKPOINT_FILE="$WORKING_LOG_DIR/checkpoints.json"
CHECKPOINT_TIME=$(date +%s%3N)

cat > "$CHECKPOINT_FILE" << EOF
{
  "checkpoints": [
    {
      "kind": "Human",
      "timestamp": $CHECKPOINT_TIME,
      "author": "$(git config user.name) <$(git config user.email)>",
      "diff_hash": "$(echo "$CHANGED_FILES" | sha256sum | cut -d' ' -f1)",
      "entries": [
        $(echo "$CHANGED_FILES" | while read file; do
            if [[ -n "$file" ]]; then
                HASH=$(sha256sum "$file" 2>/dev/null | cut -d' ' -f1 || echo "unknown")
                echo "        {\"file\": \"$file\", \"blob_sha\": \"$HASH\"}"
            fi
        done | paste -sd',')
      ],
      "line_stats": {
        "additions": 0,
        "deletions": 0,
        "additions_sloc": 0,
        "deletions_sloc": 0
      }
    }
  ]
}
EOF

echo "检查点已保存到: $CHECKPOINT_FILE"
echo ""

echo -e "${GREEN}[PRE-COMMIT] 完成!${NC}\n"

# ============================================
# 用户执行 git commit
# ============================================

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}用户执行: git commit -m 'Your message'${NC}"
echo -e "${BLUE}========================================${NC}\n"

# 这里我们不实际执行 commit，只是演示
echo "(此脚本不会真正执行 commit，仅演示操作)"
echo ""

# ============================================
# POST-COMMIT: Git Commit 之后的操作
# ============================================

echo -e "${GREEN}[POST-COMMIT] 开始执行 commit 后的操作...${NC}\n"

# 1. 获取新的 commit SHA
echo -e "${YELLOW}步骤 1: 获取新的 commit SHA${NC}"
NEW_COMMIT=$(git rev-parse HEAD)
echo "NEW_COMMIT = $NEW_COMMIT"
echo ""

# 2. 读取工作日志
echo -e "${YELLOW}步骤 2: 读取工作日志${NC}"
echo "从 $CHECKPOINT_FILE 读取检查点数据"
if [[ -f "$CHECKPOINT_FILE" ]]; then
    echo "检查点数据:"
    cat "$CHECKPOINT_FILE" | jq '.' 2>/dev/null || cat "$CHECKPOINT_FILE"
fi
echo ""

# 3. 过滤出实际被提交的文件
echo -e "${YELLOW}步骤 3: 过滤出实际被提交的文件${NC}"
echo "使用命令: git diff-tree --no-commit-id --name-only -r $NEW_COMMIT"
COMMITTED_FILES=$(git diff-tree --no-commit-id --name-only -r $NEW_COMMIT 2>/dev/null || echo "")
if [[ -n "$COMMITTED_FILES" ]]; then
    echo "实际提交的文件:"
    echo "$COMMITTED_FILES"
else
    echo "无法获取提交文件（可能是空仓库或演示模式）"
fi
echo ""

# 4. 更新 AI 提示词到最新版本
echo -e "${YELLOW}步骤 4: 更新 AI 对话记录到最新版本${NC}"
echo "对于不同的 AI 工具，读取最新的对话记录:"
echo "  - Cursor: 读取 ~/.cursor/... 下的数据库"
echo "  - Claude Code: 读取 ~/.claude-code/... 下的 JSONL 文件"
echo "  - GitHub Copilot: 读取 VS Code 扩展的会话 JSON"
echo "  - Gemini, Continue CLI: 各自特定的格式"
echo ""

# 5. 生成归属日志（Authorship Log）
echo -e "${YELLOW}步骤 5: 生成归属日志${NC}"
AUTHORSHIP_LOG_FILE="/tmp/authorship-log-$NEW_COMMIT.json"

cat > "$AUTHORSHIP_LOG_FILE" << EOF
{
  "version": "1.0",
  "metadata": {
    "base_commit_sha": "$NEW_COMMIT",
    "timestamp": $CHECKPOINT_TIME,
    "prompts": {}
  },
  "attestations": [
    {
      "file": "example.txt",
      "attributions": [
        {
          "start_line": 1,
          "end_line": 10,
          "author_id": "Human",
          "timestamp": $CHECKPOINT_TIME
        }
      ]
    }
  ]
}
EOF

echo "归属日志已生成: $AUTHORSHIP_LOG_FILE"
echo "内容预览:"
cat "$AUTHORSHIP_LOG_FILE" | jq '.' 2>/dev/null || cat "$AUTHORSHIP_LOG_FILE"
echo ""

# 6. 将归属日志附加到 git notes
echo -e "${YELLOW}步骤 6: 将归属日志附加到 git notes${NC}"
echo "执行命令: git notes --ref=git-ai add -f -m \"\$(cat $AUTHORSHIP_LOG_FILE)\" $NEW_COMMIT"
echo ""
echo "注意: 此演示脚本不会真正执行此操作以避免修改您的仓库"
echo "实际命令会将归属日志作为 git note 附加到 commit 对象"
echo ""

# 实际执行（注释掉以避免修改仓库）
# git notes --ref=git-ai add -f -m "$(cat $AUTHORSHIP_LOG_FILE)" $NEW_COMMIT

# 7. 为下次提交准备新的工作日志
echo -e "${YELLOW}步骤 7: 为下次提交准备新的工作日志${NC}"
NEW_WORKING_LOG_DIR=".git/git-ai/working-logs/$NEW_COMMIT"
mkdir -p "$NEW_WORKING_LOG_DIR/blobs"
echo "新的工作日志目录: $NEW_WORKING_LOG_DIR"
echo ""

# 8. 清理旧的工作日志
echo -e "${YELLOW}步骤 8: 清理旧的工作日志${NC}"
echo "删除: $WORKING_LOG_DIR"
# rm -rf "$WORKING_LOG_DIR" # 注释掉以保留演示数据
echo ""

# 9. 显示统计信息
echo -e "${YELLOW}步骤 9: 显示 AI 代码统计${NC}"
echo "示例输出:"
echo "Human changed 3 of 5 file(s) that have changed since the last commit"
echo ""

# 查看 git notes（如果存在）
echo -e "${YELLOW}步骤 10: 查看附加的 git notes${NC}"
echo "执行命令: git notes --ref=git-ai show $NEW_COMMIT"
git notes --ref=git-ai show $NEW_COMMIT 2>/dev/null || echo "(未找到 git-ai notes，这是正常的因为我们没有实际执行)"
echo ""

echo -e "${GREEN}[POST-COMMIT] 完成!${NC}\n"

# ============================================
# 后续查询操作
# ============================================

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}后续可以执行的查询操作${NC}"
echo -e "${BLUE}========================================${NC}\n"

echo -e "${YELLOW}1. 查看文件的 AI 代码分布:${NC}"
echo "   git-ai blame <file>"
echo "   等价于: git blame <file> + 解析 git notes 中的归属数据"
echo ""

echo -e "${YELLOW}2. 查看提交的 AI 统计:${NC}"
echo "   git-ai show <commit>"
echo "   等价于: git show <commit> + 解析该 commit 的 git notes"
echo ""

echo -e "${YELLOW}3. 查看所有 git-ai notes:${NC}"
echo "   git log --show-notes=git-ai"
echo ""

echo -e "${YELLOW}4. 导出归属数据:${NC}"
echo "   git notes --ref=git-ai show <commit> | jq '.'"
echo ""

# 清理演示文件
echo -e "${YELLOW}清理演示文件...${NC}"
rm -f "$AUTHORSHIP_LOG_FILE"

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}演示完成!${NC}"
echo -e "${GREEN}========================================${NC}"
