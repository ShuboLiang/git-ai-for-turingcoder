use crate::commands::hooks::cherry_pick_hooks;
use crate::commands::hooks::clone_hooks;
use crate::commands::hooks::commit_hooks;
use crate::commands::hooks::fetch_hooks;
use crate::commands::hooks::merge_hooks;
use crate::commands::hooks::push_hooks;
use crate::commands::hooks::rebase_hooks;
use crate::commands::hooks::reset_hooks;
use crate::commands::hooks::stash_hooks;
use crate::config;
use crate::git::cli_parser::{ParsedGitInvocation, parse_git_cli_args};
use crate::git::find_repository;
use crate::git::repository::Repository;
use crate::observability;

use crate::observability::wrapper_performance_targets::log_performance_target_if_violated;
use crate::utils::debug_log;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::process::Command;
#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

#[cfg(unix)]
static CHILD_PGID: AtomicI32 = AtomicI32::new(0);

/// Error type for hook panics
#[derive(Debug)]
struct HookPanicError(String);

impl std::fmt::Display for HookPanicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for HookPanicError {}

#[cfg(unix)]
extern "C" fn forward_signal_handler(sig: libc::c_int) {
    let pgid = CHILD_PGID.load(Ordering::Relaxed);
    if pgid > 0 {
        unsafe {
            // Send to the whole child process group
            let _ = libc::kill(-pgid, sig);
        }
    }
}

#[cfg(unix)]
fn install_forwarding_handlers() {
    unsafe {
        let handler = forward_signal_handler as usize;
        let _ = libc::signal(libc::SIGTERM, handler);
        let _ = libc::signal(libc::SIGINT, handler);
        let _ = libc::signal(libc::SIGHUP, handler);
        let _ = libc::signal(libc::SIGQUIT, handler);
    }
}

#[cfg(unix)]
fn uninstall_forwarding_handlers() {
    unsafe {
        let _ = libc::signal(libc::SIGTERM, libc::SIG_DFL);
        let _ = libc::signal(libc::SIGINT, libc::SIG_DFL);
        let _ = libc::signal(libc::SIGHUP, libc::SIG_DFL);
        let _ = libc::signal(libc::SIGQUIT, libc::SIG_DFL);
    }
}

pub struct CommandHooksContext {
    pub pre_commit_hook_result: Option<bool>,
    pub rebase_original_head: Option<String>,
    pub _rebase_onto: Option<String>,
    pub fetch_authorship_handle: Option<std::thread::JoinHandle<()>>,
    pub stash_sha: Option<String>,
    pub push_authorship_handle: Option<std::thread::JoinHandle<()>>,
}

/// 处理 git 命令的主入口函数
///
/// # 参数
/// * `args` - git 命令的参数列表（不包含 "git" 本身）
///           例如：["commit", "-m", "message"] 或 ["push", "origin", "main"]
///
/// # 功能概述
/// 这是 git-ai 的核心函数，负责拦截并增强所有 git 命令：
/// 1. **Shell 自动补全检测**：检测是否在 shell 补全上下文中，如果是则直接代理到原始 git
/// 2. **参数解析**：解析 git 命令行参数，识别命令类型和选项
/// 3. **仓库查找**：定位 git 仓库，设置可观测性上下文
/// 4. **配置检查**：检查当前仓库是否在允许列表中，决定是否启用 git-ai hooks
/// 5. **特殊命令处理**：
///    - clone：单独处理（因为仓库在命令执行前不存在）
///    - help：直接代理，不执行 hooks
/// 6. **Hook 执行流程**（当启用时）：
///    - Pre-command hooks：在 git 命令执行前运行（如记录状态、创建 checkpoint）
///    - 执行实际的 git 命令
///    - Post-command hooks：在 git 命令执行后运行（如更新归属信息、处理 rebase）
/// 7. **性能监控**：记录 pre-hook、git 执行、post-hook 的耗时，违反性能目标时记录日志
/// 8. **退出处理**：以与实际 git 命令相同的退出码退出
///
/// # 执行流程示例
/// 对于命令 `git commit -m "message"`：
/// 1. 检测不在补全上下文中
/// 2. 解析参数得到 command="commit"
/// 3. 找到当前 git 仓库
/// 4. 检查配置，确认仓库在允许列表中
/// 5. 执行 pre_commit_hook：创建 authorship checkpoint
/// 6. 代理执行 `git commit -m "message"`
/// 7. 执行 post_commit_hook：记录提交信息，更新归属日志
/// 8. 记录性能指标
/// 9. 以 git 命令的退出码退出
///
/// # 错误处理
/// - Hook 执行中的 panic 会被捕获，不会中断 git 命令执行（优雅降级）
/// - 如果 git 命令本身失败，会保留其原始退出码
/// - 所有错误都会记录到日志和可观测性系统
///
/// # 注意事项
/// - 函数永不返回（通过 exit_with_status 退出进程）
/// - 在 shell 补全上下文中会完全跳过 git-ai 逻辑
/// - clone 命令需要特殊处理（在仓库创建后执行 post-hook）
pub fn handle_git(args: &[String]) {
    // 步骤 1: 检测 Shell 自动补全上下文
    //
    // 背景说明：
    // 当你在终端输入 "git com" 然后按 Tab 键时，Shell 会调用自动补全功能
    // 此时 Shell 会执行 git 命令来获取可用的子命令列表（如 commit、config 等）
    //
    // 问题：
    // 如果让 git-ai 的增强逻辑介入补全过程，可能会：
    // 1. 降低补全速度（因为要执行额外的 hook 逻辑）
    // 2. 产生不必要的日志或副作用
    // 3. 干扰 Shell 的补全脚本解析输出
    //
    // 解决方案：
    // 通过检测环境变量（COMP_LINE、COMP_POINT 等）识别是否在补全上下文中
    // 如果是，直接透传给原始 git，完全跳过 git-ai 的所有增强功能
    // 这样用户在使用 Tab 补全时感觉不到任何差异，就像直接使用原生 git 一样
    if in_shell_completion_context() {
        let orig_args: Vec<String> = std::env::args().skip(1).collect();
        proxy_to_git(&orig_args, true);
        return;
    }

    // 步骤 2: 解析 git 命令行参数
    // 将原始参数字符串数组解析为结构化的 ParsedGitInvocation 对象
    // 包含：命令名称、全局选项、命令选项、是否为 help 请求等
    let mut parsed_args = parse_git_cli_args(args);

    // 步骤 3: 查找 git 仓库
    // 基于全局参数（如 -C、--git-dir）尝试定位 git 仓库
    // 返回 Option<Repository>，如果不在 git 仓库中则为 None
    let mut repository_option = find_repository(&parsed_args.global_args).ok();

    // 标记是否找到了有效的 git 仓库
    let has_repo = repository_option.is_some();

    // 步骤 4: 设置可观测性上下文
    // 如果找到仓库，将其信息注册到可观测性系统
    // 用于后续的日志记录和错误追踪
    if let Some(repo) = repository_option.as_ref() {
        observability::set_repo_context(repo);
    }

    // 步骤 5: 加载配置并检查仓库权限
    //
    // 加载 git-ai 的全局配置（来自 ~/.git-ai/config.json）
    // 配置内容包括：
    // - git_path: 实际 git 命令的路径（如 /usr/bin/git）
    // - allow_repositories: 允许使用 git-ai 的仓库白名单（支持 glob 模式）
    // - exclude_repositories: 排除使用 git-ai 的仓库黑名单（支持 glob 模式）
    // - ignore_prompts: 是否忽略提示（通常用于自动化场景）
    // - telemetry_oss: OSS 遥测开关（"off" 表示关闭）
    // - telemetry_enterprise_dsn: 企业版遥测数据上报地址
    // - disable_version_checks: 是否禁用版本检查
    // - disable_auto_updates: 是否禁用自动更新
    // - update_channel: 更新渠道（"latest" 或 "next"）
    // - feature_flags: 功能特性开关（如 rewrite_stash）
    let config = config::Config::get();

    // 判断当前仓库是否应该跳过 git-ai hooks
    // 检查逻辑：
    // 1. 如果仓库的 remote URL 匹配 exclude_repositories 中的任一模式 → 跳过（黑名单优先）
    // 2. 如果 allow_repositories 为空 → 允许所有仓库（除非被黑名单排除）
    // 3. 如果 allow_repositories 不为空，且仓库的 remote URL 匹配其中任一模式 → 允许
    // 4. 否则 → 跳过
    //
    // 示例配置：
    // {
    //   "allow_repositories": ["https://github.com/myorg/*"],
    //   "exclude_repositories": ["https://github.com/myorg/private-*"]
    // }
    let skip_hooks = !config.is_allowed_repository(&repository_option);

    if skip_hooks {
        debug_log("跳过 git-ai hooks，因为仓库在排除列表中或不在 allow_repositories 列表中");
    }

    // 步骤 6: 特殊处理 clone 命令
    // clone 命令比较特殊：仓库在命令执行前不存在
    // 因此需要先执行 git clone，再在新仓库中执行 post-clone hook
    if parsed_args.command.as_deref() == Some("clone") && !parsed_args.is_help && !skip_hooks {
        // 执行实际的 git clone 命令
        let exit_status = proxy_to_git(&parsed_args.to_invocation_vec(), false);
        // 在新创建的仓库中执行 post-clone hook（如初始化 git-ai 配置）
        clone_hooks::post_clone_hook(&parsed_args, exit_status);
        // 以 clone 命令的退出码退出
        exit_with_status(exit_status);
    }

    // 步骤 7: 执行带 hooks 的 git 命令（或不带 hooks）
    let exit_status = if !parsed_args.is_help && has_repo && !skip_hooks {
        // 条件满足时执行完整的 pre-hook -> git -> post-hook 流程：
        // - 不是 help 请求
        // - 找到了 git 仓库
        // - 仓库未被配置跳过

        // 初始化 hook 上下文，用于在 pre/post hooks 之间传递信息
        let mut command_hooks_context = CommandHooksContext {
            pre_commit_hook_result: None,  // commit pre-hook 的执行结果
            rebase_original_head: None,    // rebase 前的 HEAD 位置
            _rebase_onto: None,            // rebase 的目标分支
            fetch_authorship_handle: None, // fetch 归属数据的异步任务句柄
            stash_sha: None,               // stash 操作的 SHA
            push_authorship_handle: None,  // push 归属数据的异步任务句柄
        };

        let repository = repository_option.as_mut().unwrap();

        // 阶段 1: 执行 Pre-command Hooks
        let pre_command_start = Instant::now();
        run_pre_command_hooks(&mut command_hooks_context, &mut parsed_args, repository);
        let pre_command_duration = pre_command_start.elapsed();

        // 阶段 2: 代理执行实际的 git 命令
        let git_start = Instant::now();
        let exit_status = proxy_to_git(&parsed_args.to_invocation_vec(), false);
        let git_duration = git_start.elapsed();

        // 阶段 3: 执行 Post-command Hooks
        let post_command_start = Instant::now();
        run_post_command_hooks(
            &mut command_hooks_context,
            &parsed_args,
            exit_status,
            repository,
        );
        let post_command_duration = post_command_start.elapsed();

        // 步骤 8: 性能监控
        // 如果任一阶段超过预设的性能目标，记录警告日志
        log_performance_target_if_violated(
            &parsed_args.command.as_deref().unwrap_or("unknown"),
            pre_command_duration,
            git_duration,
            post_command_duration,
        );

        exit_status
    } else {
        // 直接执行 git 命令，不运行 hooks
        // 适用场景：help 请求、没有仓库、或仓库被配置跳过
        proxy_to_git(&parsed_args.to_invocation_vec(), false)
    };

    // 步骤 9: 以 git 命令的退出码退出进程
    // 确保 git-ai 的退出行为与原始 git 完全一致
    exit_with_status(exit_status);
}

/// 在 git 命令执行前运行相应的 pre-command hooks
///
/// # 参数
/// * `command_hooks_context` - 可变引用，用于在 pre/post hooks 之间传递上下文信息
/// * `parsed_args` - 可变引用，解析后的 git 命令参数
/// * `repository` - 可变引用，git 仓库对象
///
/// # 功能
/// - 根据 git 命令类型（commit, rebase, push 等）执行对应的前置钩子
/// - 使用 panic 捕获机制确保即使钩子代码出错也不会中断 git 命令执行
/// - 记录所有 panic 错误到日志和可观测性系统
fn run_pre_command_hooks(
    command_hooks_context: &mut CommandHooksContext,
    parsed_args: &mut ParsedGitInvocation,
    repository: &mut Repository,
) {
    // 使用 catch_unwind 捕获可能发生的 panic，防止整个程序崩溃
    // AssertUnwindSafe 告诉编译器这些引用在 panic 后是安全的
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 根据 git 命令类型执行对应的 pre-hook
        match parsed_args.command.as_deref() {
            // commit 命令：创建 checkpoint 记录代码归属
            Some("commit") => {
                command_hooks_context.pre_commit_hook_result = Some(
                    commit_hooks::commit_pre_command_hook(parsed_args, repository),
                );
            }
            // rebase 命令：保存 rebase 前的状态
            Some("rebase") => {
                rebase_hooks::pre_rebase_hook(parsed_args, repository, command_hooks_context);
            }
            // reset 命令：记录 reset 前的状态
            Some("reset") => {
                reset_hooks::pre_reset_hook(parsed_args, repository);
            }
            // cherry-pick 命令：记录 cherry-pick 前的状态
            Some("cherry-pick") => {
                cherry_pick_hooks::pre_cherry_pick_hook(
                    parsed_args,
                    repository,
                    command_hooks_context,
                );
            }
            // push 命令：启动异步线程处理 authorship 数据推送
            Some("push") => {
                command_hooks_context.push_authorship_handle =
                    push_hooks::push_pre_command_hook(parsed_args, repository);
            }
            // fetch/pull 命令：启动异步线程处理 authorship 数据拉取
            Some("fetch") | Some("pull") => {
                command_hooks_context.fetch_authorship_handle =
                    fetch_hooks::fetch_pull_pre_command_hook(parsed_args, repository);
            }
            // stash 命令：根据特性开关决定是否执行钩子
            Some("stash") => {
                let config = config::Config::get();

                if config.feature_flags().rewrite_stash {
                    stash_hooks::pre_stash_hook(parsed_args, repository, command_hooks_context);
                }
            }
            // 其他命令：不需要 pre-hook
            _ => {}
        }
    }));

    // 处理 panic 错误（如果发生）
    if let Err(panic_payload) = result {
        // 尝试提取可读的错误信息
        let error_message = if let Some(message) = panic_payload.downcast_ref::<&str>() {
            format!("Panic in run_pre_command_hooks: {}", message)
        } else if let Some(message) = panic_payload.downcast_ref::<String>() {
            format!("Panic in run_pre_command_hooks: {}", message)
        } else {
            "Panic in run_pre_command_hooks: unknown panic".to_string()
        };

        // 构建错误上下文信息（包含命令名和参数）
        let command_name = parsed_args.command.as_deref().unwrap_or("unknown");
        let context = serde_json::json!({
            "function": "run_pre_command_hooks",
            "command": command_name,
            "args": parsed_args.to_invocation_vec(),
        });

        // 记录错误到调试日志和可观测性系统
        debug_log(&error_message);
        observability::log_error(&HookPanicError(error_message.clone()), Some(context));

        // 注意：即使发生 panic，函数也会正常返回
        // 这确保 git-ai 的问题不会阻止用户使用 git（优雅降级）
    }
}

fn run_post_command_hooks(
    command_hooks_context: &mut CommandHooksContext,
    parsed_args: &ParsedGitInvocation,
    exit_status: std::process::ExitStatus,
    repository: &mut Repository,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Post-command hooks
        match parsed_args.command.as_deref() {
            Some("commit") => commit_hooks::commit_post_command_hook(
                parsed_args,
                exit_status,
                repository,
                command_hooks_context,
            ),
            Some("fetch") | Some("pull") => fetch_hooks::fetch_pull_post_command_hook(
                repository,
                parsed_args,
                exit_status,
                command_hooks_context,
            ),
            Some("push") => push_hooks::push_post_command_hook(
                repository,
                parsed_args,
                exit_status,
                command_hooks_context,
            ),
            Some("reset") => reset_hooks::post_reset_hook(parsed_args, repository, exit_status),
            Some("merge") => merge_hooks::post_merge_hook(parsed_args, exit_status, repository),
            Some("rebase") => rebase_hooks::handle_rebase_post_command(
                command_hooks_context,
                parsed_args,
                exit_status,
                repository,
            ),
            Some("cherry-pick") => cherry_pick_hooks::post_cherry_pick_hook(
                command_hooks_context,
                parsed_args,
                exit_status,
                repository,
            ),
            Some("stash") => {
                let config = config::Config::get();

                if config.feature_flags().rewrite_stash {
                    stash_hooks::post_stash_hook(
                        &command_hooks_context,
                        parsed_args,
                        repository,
                        exit_status,
                    );
                }
            }
            _ => {}
        }
    }));

    if let Err(panic_payload) = result {
        let error_message = if let Some(message) = panic_payload.downcast_ref::<&str>() {
            format!("Panic in run_post_command_hooks: {}", message)
        } else if let Some(message) = panic_payload.downcast_ref::<String>() {
            format!("Panic in run_post_command_hooks: {}", message)
        } else {
            "Panic in run_post_command_hooks: unknown panic".to_string()
        };

        let command_name = parsed_args.command.as_deref().unwrap_or("unknown");
        let exit_code = exit_status.code().unwrap_or(-1);
        let context = serde_json::json!({
            "function": "run_post_command_hooks",
            "command": command_name,
            "exit_code": exit_code,
            "args": parsed_args.to_invocation_vec(),
        });

        debug_log(&error_message);
        observability::log_error(&HookPanicError(error_message.clone()), Some(context));
    }
}

/// 将 git 命令代理转发到真实的 git 可执行文件
///
/// # 工作原理
/// git-ai 作为 git 的"包装器"(wrapper)工作：
/// 1. **查找真实 git**：通过 `config.git_cmd()` 获取真实 git 的路径
///    - 优先使用配置文件 `~/.git-ai/config.json` 中的 `git_path`
///    - 如果未配置，则探测常见位置：
///      * macOS: /opt/homebrew/bin/git, /usr/local/bin/git
///      * Linux: /usr/bin/git, /bin/git
///      * Windows: C:\Program Files\Git\bin\git.exe
///    - 如果都找不到，程序会报错退出
///
/// 2. **命令替换**：将用户执行的 `git` 替换为真实 git 的完整路径
///    例如：用户执行 `git commit -m "msg"`
///    → git-ai 拦截后执行 `/usr/bin/git commit -m "msg"`
///
/// 3. **进程管理**：
///    - Unix: 为非交互式命令创建新进程组，便于信号传递
///    - 交互式命令（如 rebase -i）保持在前台进程组，避免挂起
///    - 安装信号转发处理器，确保 Ctrl+C 等信号正确传递给子进程
///
/// # 参数
/// * `args` - 要传递给真实 git 的参数列表
/// * `exit_on_completion` - 是否在命令完成后立即退出进程
///                          true: 直接以 git 的退出码退出（用于补全等场景）
///                          false: 返回退出状态供调用者处理
///
/// # 返回值
/// 真实 git 命令的退出状态（ExitStatus）
///
/// # 示例
/// ```
/// // 用户执行: git commit -m "fix bug"
/// // git-ai 拦截后调用:
/// proxy_to_git(&["commit", "-m", "fix bug"], false)
/// // 实际执行: /usr/bin/git commit -m "fix bug"
/// ```
fn proxy_to_git(args: &[String], exit_on_completion: bool) -> std::process::ExitStatus {
    // 获取真实 git 路径和来源信息并打印
    let config = config::Config::get();
    let git_path = config.git_cmd();
    let git_source = config.git_cmd_source();
    eprintln!("[git-ai] 真实 git 路径: {}", git_path);
    eprintln!("[git-ai] 查找方式: {}", git_source);

    // 使用 spawn 方式启动子进程，支持交互式命令（如 rebase -i、commit 编辑器等）
    let child = {
        #[cfg(unix)]
        {
            // Only create a new process group for non-interactive runs.
            // If stdin is a TTY, the child must remain in the foreground
            // terminal process group to avoid SIGTTIN/SIGTTOU hangs.
            let is_interactive = unsafe { libc::isatty(libc::STDIN_FILENO) == 1 };
            let should_setpgid = !is_interactive;

            let mut cmd = Command::new(config::Config::get().git_cmd());
            cmd.args(args);
            unsafe {
                let setpgid_flag = should_setpgid;
                cmd.pre_exec(move || {
                    if setpgid_flag {
                        // Make the child its own process group leader so we can signal the group
                        let _ = libc::setpgid(0, 0);
                    }
                    Ok(())
                });
            }
            // We return both the spawned child and whether we changed PGID
            match cmd.spawn() {
                Ok(child) => Ok((child, should_setpgid)),
                Err(e) => Err(e),
            }
        }
        #[cfg(not(unix))]
        {
            Command::new(config::Config::get().git_cmd())
                .args(args)
                .spawn()
        }
    };

    #[cfg(unix)]
    match child {
        Ok((mut child, setpgid)) => {
            #[cfg(unix)]
            {
                if setpgid {
                    // Record the child's process group id (same as its pid after setpgid)
                    let pgid: i32 = child.id() as i32;
                    CHILD_PGID.store(pgid, Ordering::Relaxed);
                    install_forwarding_handlers();
                }
            }
            let status = child.wait();
            match status {
                Ok(status) => {
                    #[cfg(unix)]
                    {
                        if setpgid {
                            CHILD_PGID.store(0, Ordering::Relaxed);
                            uninstall_forwarding_handlers();
                        }
                    }
                    if exit_on_completion {
                        exit_with_status(status);
                    }
                    return status;
                }
                Err(e) => {
                    #[cfg(unix)]
                    {
                        if setpgid {
                            CHILD_PGID.store(0, Ordering::Relaxed);
                            uninstall_forwarding_handlers();
                        }
                    }
                    eprintln!("Failed to wait for git process: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to execute git command: {}", e);
            std::process::exit(1);
        }
    }

    #[cfg(not(unix))]
    match child {
        Ok(mut child) => {
            let status = child.wait();
            match status {
                Ok(status) => {
                    if exit_on_completion {
                        exit_with_status(status);
                    }
                    return status;
                }
                Err(e) => {
                    eprintln!("Failed to wait for git process: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to execute git command: {}", e);
            std::process::exit(1);
        }
    }
}

// Exit mirroring the child's termination: same signal if signaled, else exit code
fn exit_with_status(status: std::process::ExitStatus) -> ! {
    #[cfg(unix)]
    {
        if let Some(sig) = status.signal() {
            unsafe {
                libc::signal(sig, libc::SIG_DFL);
                libc::raise(sig);
            }
            // Should not return
            unreachable!();
        }
    }
    std::process::exit(status.code().unwrap_or(1));
}

// Detect if current process invocation is coming from shell completion machinery
// (bash, zsh via bashcompinit). If so, we should proxy directly to the real git
// without any extra behavior that could interfere with completion scripts.
fn in_shell_completion_context() -> bool {
    std::env::var("COMP_LINE").is_ok()
        || std::env::var("COMP_POINT").is_ok()
        || std::env::var("COMP_TYPE").is_ok()
}
