use crate::authorship::pre_commit;
use crate::commands::git_handlers::CommandHooksContext;
use crate::git::cli_parser::{ParsedGitInvocation, is_dry_run};
use crate::git::repository::Repository;
use crate::git::rewrite_log::RewriteLogEvent;
use crate::utils::debug_log;

pub fn commit_pre_command_hook(
    parsed_args: &ParsedGitInvocation,
    repository: &mut Repository,
) -> bool {
    if is_dry_run(&parsed_args.command_args) {
        return false;
    }

    // store HEAD context for post-command hook
    repository.require_pre_command_head();

    let default_author = get_commit_default_author(&repository, &parsed_args.command_args);

    // Run pre-commit logic
    if let Err(e) = pre_commit::pre_commit(&repository, default_author.clone()) {
        if e.to_string()
            .contains("Cannot run checkpoint on bare repositories")
        {
            eprintln!(
                "Cannot run checkpoint on bare repositories (skipping git-ai pre-commit hook)"
            );
            return false;
        }
        eprintln!("Pre-commit failed: {}", e);
        std::process::exit(1);
    }
    return true;
}

/// commit 命令的后置钩子函数
///
/// # 参数
/// * `parsed_args` - 解析后的 git 命令参数
/// * `exit_status` - git commit 命令的退出状态
/// * `repository` - 可变引用的 git 仓库对象
/// * `command_hooks_context` - 命令钩子上下文，包含 pre-commit 的执行结果
///
/// # 功能说明
/// 此函数在 `git commit` 命令执行完成后被调用，主要完成以下工作：
/// 1. 验证提交是否成功（检查 dry-run、exit_status、pre-commit 结果）
/// 2. 获取提交前后的 commit SHA，用于追踪提交历史
/// 3. 处理 rewrite log 事件，记录提交或修改提交(amend)的操作
/// 4. 将工作日志(working log)转换为归属日志(authorship log)，完成代码归属追踪
pub fn commit_post_command_hook(
    parsed_args: &ParsedGitInvocation,
    exit_status: std::process::ExitStatus,
    repository: &mut Repository,
    command_hooks_context: &mut CommandHooksContext,
) {
    // 检查是否为 dry-run 模式
    // dry-run 模式下提交不会真正执行，因此跳过后置钩子
    if is_dry_run(&parsed_args.command_args) {
        return;
    }

    // 检查 git commit 命令是否执行成功
    // 如果提交失败（如有冲突、空提交等），则跳过后置钩子
    if !exit_status.success() {
        return;
    }

    // 检查 pre-commit 钩子的执行结果
    // 如果 pre-commit 失败，说明 checkpoint 创建失败，后续处理无法进行
    if let Some(pre_commit_hook_result) = command_hooks_context.pre_commit_hook_result {
        if !pre_commit_hook_result {
            debug_log("Skipping git-ai post-commit hook because pre-commit hook failed");
            return;
        }
    }

    // 检查是否需要抑制输出
    // 当用户使用 --porcelain、--quiet 等标志时，应保持输出静默
    let supress_output = parsed_args.has_command_flag("--porcelain")
        || parsed_args.has_command_flag("--quiet")
        || parsed_args.has_command_flag("-q")
        || parsed_args.has_command_flag("--no-status");

    // 获取提交前的 commit SHA（在 pre-commit 钩子中保存）
    // 这用于追踪提交历史和处理 rewrite 事件
    let original_commit = repository.pre_command_base_commit.clone();

    // 获取新提交的 SHA（提交后的 HEAD 指向）
    let new_sha = repository.head().ok().map(|h| h.target().ok()).flatten();

    // 处理空仓库的情况
    // 如果 new_sha 为 None，说明仓库仍然为空（首次提交失败），跳过后续处理
    if new_sha.is_none() {
        return;
    }

    // 获取提交作者信息
    // 这将用于记录归属日志中的作者
    let commit_author = get_commit_default_author(repository, &parsed_args.command_args);

    // 根据是否为 amend 提交，创建不同类型的 rewrite log 事件
    if parsed_args.has_command_flag("--amend") && original_commit.is_some() && new_sha.is_some() {
        // amend 提交：修改已有提交
        // 记录 commit_amend 事件，包含原始提交和新提交的 SHA
        // 这对于追踪修改历史和维护代码归属的准确性至关重要
        repository.handle_rewrite_log_event(
            RewriteLogEvent::commit_amend(original_commit.unwrap(), new_sha.unwrap()),
            commit_author,
            supress_output,
            true, // 表示这是一个 commit 操作，需要将 working log 转换为 authorship log
        );
    } else {
        // 普通提交：创建新提交
        // 记录 commit 事件，original_commit 可能为 None（首次提交）或 Some（常规提交）
        repository.handle_rewrite_log_event(
            RewriteLogEvent::commit(original_commit, new_sha.unwrap()),
            commit_author,
            supress_output,
            true, // 表示这是一个 commit 操作，需要将 working log 转换为 authorship log
        );
    }
    // 注意：handle_rewrite_log_event 的最后一个参数为 true 时，
    // 会将工作日志(working log)转换为归属日志(authorship log)，
    // 这是 git-ai 完成代码归属追踪的关键步骤
}

pub fn get_commit_default_author(repo: &Repository, args: &[String]) -> String {
    // According to git commit manual, --author flag overrides all other author information
    if let Some(author_spec) = extract_author_from_args(args) {
        if let Ok(Some(resolved_author)) = repo.resolve_author_spec(&author_spec) {
            if !resolved_author.trim().is_empty() {
                return resolved_author.trim().to_string();
            }
        }
    }

    // Normal precedence when --author is not specified:
    // Name precedence: GIT_AUTHOR_NAME env > user.name config > extract from EMAIL env > "unknown"
    // Email precedence: GIT_AUTHOR_EMAIL env > user.email config > EMAIL env > None

    let mut author_name: Option<String> = None;
    let mut author_email: Option<String> = None;

    // Check GIT_AUTHOR_NAME environment variable
    if let Ok(name) = std::env::var("GIT_AUTHOR_NAME") {
        if !name.trim().is_empty() {
            author_name = Some(name.trim().to_string());
        }
    }

    // Fall back to git config user.name
    if author_name.is_none() {
        if let Ok(Some(name)) = repo.config_get_str("user.name") {
            if !name.trim().is_empty() {
                author_name = Some(name.trim().to_string());
            }
        }
    }

    // Check GIT_AUTHOR_EMAIL environment variable
    if let Ok(email) = std::env::var("GIT_AUTHOR_EMAIL") {
        if !email.trim().is_empty() {
            author_email = Some(email.trim().to_string());
        }
    }

    // Fall back to git config user.email
    if author_email.is_none() {
        if let Ok(Some(email)) = repo.config_get_str("user.email") {
            if !email.trim().is_empty() {
                author_email = Some(email.trim().to_string());
            }
        }
    }

    // Check EMAIL environment variable as fallback for both name and email
    if author_name.is_none() || author_email.is_none() {
        if let Ok(email) = std::env::var("EMAIL") {
            if !email.trim().is_empty() {
                // Extract name part from email if we don't have a name yet
                if author_name.is_none() {
                    if let Some(at_pos) = email.find('@') {
                        let name_part = &email[..at_pos];
                        if !name_part.is_empty() {
                            author_name = Some(name_part.to_string());
                        }
                    }
                }
                // Use as email if we don't have an email yet
                if author_email.is_none() {
                    author_email = Some(email.trim().to_string());
                }
            }
        }
    }

    // Format the author string based on what we have
    match (author_name, author_email) {
        (Some(name), Some(email)) => format!("{} <{}>", name, email),
        (Some(name), None) => name,
        (None, Some(email)) => email,
        (None, None) => {
            eprintln!("Warning: No author information found. Using 'unknown' as author.");
            "unknown".to_string()
        }
    }
}

fn extract_author_from_args(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        // Handle --author=<author> format
        if let Some(author_value) = arg.strip_prefix("--author=") {
            return Some(author_value.to_string());
        }

        // Handle --author <author> format (separate arguments)
        if arg == "--author" && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }

        i += 1;
    }
    None
}
