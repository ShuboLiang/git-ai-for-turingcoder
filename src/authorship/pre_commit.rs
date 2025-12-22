use crate::authorship::working_log::CheckpointKind;
use crate::error::GitAiError;
use crate::git::repository::Repository;

/// pre-commit 钩子函数：在提交前创建人工编辑检查点
///
/// # 参数
/// * `repo` - git 仓库引用
/// * `default_author` - 默认作者信息（格式："Name <email>"）
///
/// # 返回值
/// * `Result<(), GitAiError>` - 成功返回 ()，失败返回 GitAiError
///
/// # 功能说明
/// 此函数是 git-ai 代码归属追踪系统的核心入口之一，在 git commit 执行前被调用。
/// 主要作用是创建一个"人工编辑检查点"(Human checkpoint)，用于：
/// 1. 标记哪些代码修改是由人工编辑完成的（非 AI 生成）
/// 2. 与之前的 AI 检查点进行对比，确定代码归属
/// 3. 为后续的 blame 功能提供准确的归属数据
///
/// # CheckpointKind::Human 的含义
/// - 表示这是一个人工编辑检查点，用于区分 AI 生成的代码和人工修改的代码
/// - 通过对比人工检查点与 AI 检查点的差异，可以准确判断每行代码的来源
pub fn pre_commit(repo: &Repository, default_author: String) -> Result<(), GitAiError> {
    // 运行 checkpoint 命令创建人工编辑检查点
    // 参数说明：
    // - repo: 仓库对象
    // - default_author: 提交作者信息
    // - CheckpointKind::Human: 检查点类型为"人工编辑"，与 AI 检查点区分
    // - false: 不启用详细输出模式
    // - false: 不强制覆盖已存在的检查点
    // - true: 启用静默模式，减少输出信息
    // - None: 不指定特定的文件路径（处理所有变更）
    // - true: 如果没有 AI 检查点则跳过
    //         这是一个优化：如果用户从未使用 AI 助手，就不需要创建人工检查点
    //         TODO: 注意这里有个已知 bug，关于清理状态的问题，INITIAL 检查点可能未被正确删除
    let result: Result<(usize, usize, usize), GitAiError> = crate::commands::checkpoint::run(
        repo,
        &default_author,
        CheckpointKind::Human,
        false,
        false,
        true,
        None,
        true,
    );

    // 将 checkpoint::run 的返回值 (usize, usize, usize) 映射为 ()
    // 这三个 usize 分别表示：添加的行数、删除的行数、修改的行数
    // 对于 pre_commit 调用者而言，只需要知道成功或失败，不需要具体统计信息
    result.map(|_| ())
}
