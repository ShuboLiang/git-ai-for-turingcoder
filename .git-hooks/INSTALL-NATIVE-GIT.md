# 在原生 Git 中使用 git-ai 工具

如果您想使用**原生 git 命令**（而不是 git-ai 的包装器），但仍然获得 git-ai 的 AI 代码追踪功能，可以通过 Git hooks 来实现。

## 🎯 目标

- ✅ 使用原生 `git commit`、`git push` 等命令
- ✅ 自动调用 git-ai 进行 AI 代码追踪
- ✅ 无需使用 git-ai 作为 git 的包装器

## 📦 安装步骤

### 步骤 1：确保已安装 git-ai

```bash
# Mac/Linux/WSL
curl -sSL https://raw.githubusercontent.com/acunniffe/git-ai/main/install.sh | bash

# Windows (PowerShell)
# 参考官方文档安装
```

### 步骤 2：安装 Git hooks

```bash
# Linux/Mac/WSL
cp .git-hooks/pre-commit-gitai .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit

cp .git-hooks/post-commit-gitai .git/hooks/post-commit
chmod +x .git/hooks/post-commit
```

```powershell
# Windows PowerShell
Copy-Item .git-hooks\pre-commit-gitai .git\hooks\pre-commit
Copy-Item .git-hooks\post-commit-gitai .git\hooks\post-commit
```

## 🔧 工作原理

### Pre-Commit Hook

```bash
# 在每次 git commit 前自动运行
git-ai checkpoint --kind Human --quiet
```

这会创建一个 Human 检查点，追踪您在 commit 前的代码变更。

### Post-Commit Hook

git-ai 的 post-commit 处理已经内置在工具中，通常不需要额外的 hook。但如果需要，可以手动触发：

```bash
git-ai sync-authorship
```

## 💡 使用示例

安装 hooks 后，您可以完全使用原生 Git 命令：

```bash
# 1. 修改代码
echo "console.log('Hello')" >> app.js

# 2. 使用原生git命令提交
git add app.js
git commit -m "Add hello message"
# 🔍 Hook会自动调用: git-ai checkpoint --kind Human --quiet

# 3. 查看AI代码分布
git-ai blame app.js

# 4. 查看提交的AI统计
git-ai show HEAD

# 5. 正常推送
git push origin main
```

## 🔄 完整对比

### 使用 git-ai 包装器（默认方式）

```bash
# git-ai拦截所有git命令
git commit -m "message"  # 实际执行: git-ai commit -m "message"
```

### 使用原生 Git + Hooks（您想要的方式）

```bash
# 直接使用原生git
git commit -m "message"  # 真正的 git commit
# hooks自动调用: git-ai checkpoint
```

## ⚙️ 高级配置

### 自定义 pre-commit 行为

编辑 `.git/hooks/pre-commit`:

```bash
#!/bin/sh

# 只在工作时间启用追踪
HOUR=$(date +%H)
if [ $HOUR -ge 9 ] && [ $HOUR -le 18 ]; then
    git-ai checkpoint --kind Human --quiet
fi

exit 0
```

### 添加其他检查

```bash
#!/bin/sh

# 先运行代码检查
npm run lint || exit 1

# 然后运行git-ai追踪
git-ai checkpoint --kind Human --quiet || exit 0

exit 0
```

## 📊 关键命令说明

| git-ai 命令                        | 说明                 | 适用场景           |
| ---------------------------------- | -------------------- | ------------------ |
| `git-ai checkpoint --kind Human`   | 创建人类编辑检查点   | Pre-commit hook    |
| `git-ai checkpoint --kind AiAgent` | 创建 AI 代理检查点   | AI 工具集成        |
| `git-ai sync-authorship`           | 同步归属信息         | Post-commit (可选) |
| `git-ai blame <file>`              | 查看文件 AI 代码分布 | 日常使用           |
| `git-ai show <commit>`             | 查看提交 AI 统计     | 日常使用           |

## ⚠️ 注意事项

1. **兼容性**：这种方式与 git-ai 包装器不冲突，可以混用
2. **AI 工具集成**：Cursor、Claude 等 AI 工具仍需按官方文档配置
3. **性能**：hooks 会略微增加 commit 时间（通常<100ms）
4. **Git 命令**：除了 commit，其他 git 命令（push、pull 等）不需要 hooks

## 🚀 推荐工作流

```bash
# 1. 使用原生git进行日常操作
git status
git add .
git commit -m "Your message"  # hooks自动运行
git push

# 2. 使用git-ai命令查看AI代码
git-ai blame src/app.js
git-ai show HEAD
git-ai stats

# 3. 如果AI工具（Cursor/Claude）生成代码，它们会自动创建AI检查点
# 您无需手动干预
```

## ✅ 验证安装

```bash
# 检查hooks是否正确安装
ls -la .git/hooks/pre-commit
ls -la .git/hooks/post-commit

# 测试是否工作
echo "test" >> test.txt
git add test.txt
git commit -m "Test hooks"
# 应该看到git-ai的输出信息

# 查看结果
git-ai show HEAD
```

## 🔄 卸载

如果不想使用 hooks，直接删除即可：

```bash
rm .git/hooks/pre-commit
rm .git/hooks/post-commit
```

这不会影响 git-ai 工具本身，只是不再通过 hooks 自动调用。

---

**总结**：通过这种方式，您可以 100%使用原生 Git 命令，同时通过 hooks 自动获得 git-ai 的 AI 代码追踪功能！
