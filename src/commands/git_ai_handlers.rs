use clap::builder::Str;

use crate::authorship::range_authorship;
use crate::authorship::stats::stats_command;
use crate::authorship::working_log::{AgentId, CheckpointKind};
use crate::commands;
use crate::commands::checkpoint_agent::agent_presets::{
    AgentCheckpointFlags, AgentCheckpointPreset, AgentRunResult, AiTabPreset, ClaudePreset,
    ContinueCliPreset, CursorPreset, GeminiPreset, GithubCopilotPreset,
};
use crate::commands::checkpoint_agent::agent_v1_preset::AgentV1Preset;
use crate::config;
use crate::git::find_repository;
use crate::git::find_repository_in_path;
use crate::git::repository::CommitRange;
use crate::observability;
use crate::observability::wrapper_performance_targets::log_performance_for_checkpoint;
use std::env;
use std::io::IsTerminal;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn handle_git_ai(args: &[String]) {
    if args.is_empty() {
        print_help();
        return;
    }

    let current_dir = env::current_dir().unwrap().to_string_lossy().to_string();
    let repository_option = find_repository_in_path(&current_dir).ok();

    // Set repo context to flush buffered events
    if let Some(repo) = repository_option.as_ref() {
        observability::set_repo_context(repo);
    }

    let config = config::Config::get();

    let allowed_repository = config.is_allowed_repository(&repository_option);

    match args[0].as_str() {
        "help" | "--help" | "-h" => {
            print_help();
        }
        "version" | "--version" | "-v" => {
            if cfg!(debug_assertions) {
                println!("{} (debug)", env!("CARGO_PKG_VERSION"));
            } else {
                println!(env!("CARGO_PKG_VERSION"));
            }
            std::process::exit(0);
        }
        "stats" => {
            handle_stats(&args[1..]);
        }
        "show" => {
            commands::show::handle_show(&args[1..]);
        }
        "checkpoint" => {
            if !allowed_repository {
                eprintln!(
                    "Skipping checkpoint because repository is excluded or not in allow_repositories list"
                );
                std::process::exit(1);
            }
            handle_checkpoint(&args[1..]);
        }
        "blame" => {
            handle_ai_blame(&args[1..]);
        }
        "diff" => {
            handle_ai_diff(&args[1..]);
        }
        "git-path" => {
            let config = config::Config::get();
            println!("{}", config.git_cmd());
            std::process::exit(0);
        }
        "install-hooks" => {
            if let Err(e) = commands::install_hooks::run(&args[1..]) {
                eprintln!("Install hooks failed: {}", e);
                std::process::exit(1);
            }
        }
        "squash-authorship" => {
            commands::squash_authorship::handle_squash_authorship(&args[1..]);
        }
        "ci" => {
            commands::ci_handlers::handle_ci(&args[1..]);
        }
        "upgrade" => {
            commands::upgrade::run_with_args(&args[1..]);
        }
        "flush-logs" => {
            commands::flush_logs::handle_flush_logs(&args[1..]);
        }
        "show-prompt" => {
            commands::show_prompt::handle_show_prompt(&args[1..]);
        }
        "myhelp" => {
            handle_myhelp();
        }
        "proxy" => {
            // 直接调用 handle_git，传入剩余参数
            let args: Vec<String> = args
                .iter()
                .cloned()
                .chain(std::iter::once("--no-verify".to_string()))
                .collect();
            let args = commands::git_handlers::handle_git(&args[1..]);
        }
        _ => {
            println!("Unknown git-ai command: {}", args[0]);
            std::process::exit(1);
        }
    }
}

fn print_help() {
    eprintln!("git-ai - git proxy with AI authorship tracking");
    eprintln!("");
    eprintln!("Usage: git-ai <command> [args...]");
    eprintln!("");
    eprintln!("Commands:");
    eprintln!("  checkpoint         Checkpoint working changes and attribute author");
    eprintln!("    Presets: claude, continue-cli, cursor, gemini, github-copilot, ai_tab, mock_ai");
    eprintln!(
        "    --hook-input <json|stdin>   JSON payload required by presets, or 'stdin' to read from stdin"
    );
    eprintln!("    --show-working-log          Display current working log");
    eprintln!("    --reset                     Reset working log");
    eprintln!("    mock_ai [pathspecs...]      Test preset accepting optional file pathspecs");
    eprintln!("  blame <file>       Git blame with AI authorship overlay");
    eprintln!("  diff <commit|range>  Show diff with AI authorship annotations");
    eprintln!("    <commit>              Diff from commit's parent to commit");
    eprintln!("    <commit1>..<commit2>  Diff between two commits");
    eprintln!("  stats [commit]     Show AI authorship statistics for a commit");
    eprintln!("    --json                 Output in JSON format");
    eprintln!("  show <rev|range>   Display authorship logs for a revision or range");
    eprintln!("  show-prompt <id>   Display a prompt record by its ID");
    eprintln!("    --commit <rev>        Look in a specific commit only");
    eprintln!(
        "    --offset <n>          Skip n occurrences (0 = most recent, mutually exclusive with --commit)"
    );
    eprintln!("  install-hooks      Install git hooks for AI authorship tracking");
    eprintln!("  ci                 Continuous integration utilities");
    eprintln!("    github                 GitHub CI helpers");
    eprintln!("  squash-authorship  Generate authorship log for squashed commits");
    eprintln!(
        "    <base_branch> <new_sha> <old_sha>  Required: base branch, new commit SHA, old commit SHA"
    );
    eprintln!("    --dry-run             Show what would be done without making changes");
    eprintln!("  git-path           Print the path to the underlying git executable");
    eprintln!("  upgrade            Check for updates and install if available");
    eprintln!("    --force               Reinstall latest version even if already up to date");
    eprintln!("  proxy <git-command>  Proxy git command with git-ai hooks");
    eprintln!("    Example: git-ai proxy commit -m \"message\"");
    eprintln!("  version, -v, --version     Print the git-ai version");
    eprintln!("  help, -h, --help           Show this help message");
    eprintln!("");
    std::process::exit(0);
}

fn handle_checkpoint(args: &[String]) {
    let mut repository_working_dir = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Parse checkpoint-specific arguments
    let mut show_working_log = false;
    let mut reset = false;
    let mut hook_input = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--show-working-log" => {
                show_working_log = true;
                i += 1;
            }
            "--reset" => {
                reset = true;
                i += 1;
            }
            "--hook-input" => {
                if i + 1 < args.len() {
                    hook_input = Some(args[i + 1].clone());
                    if hook_input.as_ref().unwrap() == "stdin" {
                        let mut stdin = std::io::stdin();
                        let mut buffer = String::new();
                        if let Err(e) = stdin.read_to_string(&mut buffer) {
                            eprintln!("Failed to read stdin for hook input: {}", e);
                            std::process::exit(1);
                        }
                        if !buffer.trim().is_empty() {
                            hook_input = Some(buffer);
                        } else {
                            eprintln!("No hook input provided (via --hook-input or stdin).");
                            std::process::exit(1);
                        }
                    } else if hook_input.as_ref().unwrap().trim().is_empty() {
                        eprintln!("Error: --hook-input requires a value");
                        std::process::exit(1);
                    }
                    i += 2;
                } else {
                    eprintln!("Error: --hook-input requires a value or 'stdin' to read from stdin");
                    std::process::exit(1);
                }
            }

            _ => {
                i += 1;
            }
        }
    }

    let mut agent_run_result = None;
    // Handle preset arguments after parsing all flags
    if !args.is_empty() {
        match args[0].as_str() {
            "claude" => {
                match ClaudePreset.run(AgentCheckpointFlags {
                    hook_input: hook_input.clone(),
                }) {
                    Ok(agent_run) => {
                        if agent_run.repo_working_dir.is_some() {
                            repository_working_dir = agent_run.repo_working_dir.clone().unwrap();
                        }
                        agent_run_result = Some(agent_run);
                    }
                    Err(e) => {
                        eprintln!("Claude preset error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "gemini" => {
                match GeminiPreset.run(AgentCheckpointFlags {
                    hook_input: hook_input.clone(),
                }) {
                    Ok(agent_run) => {
                        if agent_run.repo_working_dir.is_some() {
                            repository_working_dir = agent_run.repo_working_dir.clone().unwrap();
                        }
                        agent_run_result = Some(agent_run);
                    }
                    Err(e) => {
                        eprintln!("Gemini preset error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "continue-cli" => {
                match ContinueCliPreset.run(AgentCheckpointFlags {
                    hook_input: hook_input.clone(),
                }) {
                    Ok(agent_run) => {
                        if agent_run.repo_working_dir.is_some() {
                            repository_working_dir = agent_run.repo_working_dir.clone().unwrap();
                        }
                        agent_run_result = Some(agent_run);
                    }
                    Err(e) => {
                        eprintln!("Continue CLI preset error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "cursor" => {
                match CursorPreset.run(AgentCheckpointFlags {
                    hook_input: hook_input.clone(),
                }) {
                    Ok(agent_run) => {
                        if agent_run.repo_working_dir.is_some() {
                            repository_working_dir = agent_run.repo_working_dir.clone().unwrap();
                        }
                        agent_run_result = Some(agent_run);
                    }
                    Err(e) => {
                        eprintln!("Error running Cursor preset: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "github-copilot" => {
                match GithubCopilotPreset.run(AgentCheckpointFlags {
                    hook_input: hook_input.clone(),
                }) {
                    Ok(agent_run) => {
                        agent_run_result = Some(agent_run);
                    }
                    Err(e) => {
                        eprintln!("Github Copilot preset error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "ai_tab" => {
                match AiTabPreset.run(AgentCheckpointFlags {
                    hook_input: hook_input.clone(),
                }) {
                    Ok(agent_run) => {
                        if agent_run.repo_working_dir.is_some() {
                            repository_working_dir = agent_run.repo_working_dir.clone().unwrap();
                        }
                        agent_run_result = Some(agent_run);
                    }
                    Err(e) => {
                        eprintln!("ai_tab preset error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "agent-v1" => {
                match AgentV1Preset.run(AgentCheckpointFlags {
                    hook_input: hook_input.clone(),
                }) {
                    Ok(agent_run) => {
                        agent_run_result = Some(agent_run);
                    }
                    Err(e) => {
                        eprintln!("Agent V1 preset error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "mock_ai" => {
                let mock_agent_id = format!(
                    "ai-thread-{}",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or_else(|_| 0)
                );

                // Collect all remaining args (after mock_ai and flags) as pathspecs
                let edited_filepaths = if args.len() > 1 {
                    let mut paths = Vec::new();
                    for arg in &args[1..] {
                        // Skip flags
                        if !arg.starts_with("--") {
                            paths.push(arg.clone());
                        }
                    }
                    if paths.is_empty() { None } else { Some(paths) }
                } else {
                    let working_dir = agent_run_result
                        .as_ref()
                        .and_then(|r| r.repo_working_dir.clone())
                        .unwrap_or(repository_working_dir.clone());
                    // Find the git repository
                    Some(get_all_files_for_mock_ai(&working_dir))
                };

                agent_run_result = Some(AgentRunResult {
                    agent_id: AgentId {
                        tool: "mock_ai".to_string(),
                        id: mock_agent_id,
                        model: "unknown".to_string(),
                    },
                    agent_metadata: None,
                    checkpoint_kind: CheckpointKind::AiAgent,
                    transcript: None,
                    repo_working_dir: None,
                    edited_filepaths,
                    will_edit_filepaths: None,
                    dirty_files: None,
                });
            }
            _ => {}
        }
    }

    let final_working_dir = agent_run_result
        .as_ref()
        .and_then(|r| r.repo_working_dir.clone())
        .unwrap_or_else(|| repository_working_dir);
    // Find the git repository
    let repo = match find_repository_in_path(&final_working_dir) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Failed to find repository: {}", e);
            std::process::exit(1);
        }
    };

    let checkpoint_kind = agent_run_result
        .as_ref()
        .map(|r| r.checkpoint_kind)
        .unwrap_or(CheckpointKind::Human);

    if CheckpointKind::Human == checkpoint_kind && agent_run_result.is_none() {
        // Parse pathspecs after `--` for human checkpoints
        let will_edit_filepaths = if let Some(separator_pos) = args.iter().position(|a| a == "--") {
            let paths: Vec<String> = args[separator_pos + 1..]
                .iter()
                .filter(|arg| !arg.starts_with("--"))
                .cloned()
                .collect();
            if paths.is_empty() { None } else { Some(paths) }
        } else {
            Some(get_all_files_for_mock_ai(&final_working_dir))
        };

        agent_run_result = Some(AgentRunResult {
            agent_id: AgentId {
                tool: "mock_ai".to_string(),
                id: format!(
                    "ai-thread-{}",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or_else(|_| 0)
                ),
                model: "unknown".to_string(),
            },
            agent_metadata: None,
            checkpoint_kind: CheckpointKind::Human,
            transcript: None,
            will_edit_filepaths: Some(will_edit_filepaths.unwrap_or_default()),
            edited_filepaths: None,
            repo_working_dir: Some(final_working_dir),
            dirty_files: None,
        });
    }

    // Get the current user name from git config
    let default_user_name = match repo.config_get_str("user.name") {
        Ok(Some(name)) if !name.trim().is_empty() => name,
        _ => {
            eprintln!("Warning: git user.name not configured. Using 'unknown' as author.");
            "unknown".to_string()
        }
    };

    let checkpoint_start = std::time::Instant::now();
    let agent_tool = agent_run_result.as_ref().map(|r| r.agent_id.tool.clone());
    let checkpoint_result = commands::checkpoint::run(
        &repo,
        &default_user_name,
        checkpoint_kind,
        show_working_log,
        reset,
        false,
        agent_run_result,
        false,
    );
    match checkpoint_result {
        Ok((_, files_edited, _)) => {
            let elapsed = checkpoint_start.elapsed();
            log_performance_for_checkpoint(files_edited, elapsed, checkpoint_kind);
            eprintln!("Checkpoint completed in {:?}", elapsed);
        }
        Err(e) => {
            let elapsed = checkpoint_start.elapsed();
            eprintln!("Checkpoint failed after {:?} with error {}", elapsed, e);
            let context = serde_json::json!({
                "function": "checkpoint",
                "agent": agent_tool.unwrap_or_default(),
                "duration": elapsed.as_millis(),
                "checkpoint_kind": format!("{:?}", checkpoint_kind)
            });
            observability::log_error(&e, Some(context));
            std::process::exit(1);
        }
    }
}

fn handle_ai_blame(args: &[String]) {
    if args.is_empty() {
        eprintln!("Error: blame requires a file argument");
        std::process::exit(1);
    }

    // Find the git repository from current directory
    let current_dir = env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();
    let repo = match find_repository_in_path(&current_dir) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Failed to find repository: {}", e);
            std::process::exit(1);
        }
    };

    // Parse blame arguments
    let (file_path, options) = match commands::blame::parse_blame_args(args) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to parse blame arguments: {}", e);
            std::process::exit(1);
        }
    };

    // Check if this is an interactive terminal
    let is_interactive = std::io::stdout().is_terminal();

    if is_interactive && options.incremental {
        // For incremental mode in interactive terminal, we need special handling
        // This would typically involve a pager like less
        eprintln!("Error: incremental mode is not supported in interactive terminal");
        std::process::exit(1);
    }

    if let Err(e) = repo.blame(&file_path, &options) {
        eprintln!("Blame failed: {}", e);
        std::process::exit(1);
    }
}

fn handle_ai_diff(args: &[String]) {
    let current_dir = env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();
    let repo = match find_repository_in_path(&current_dir) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Failed to find repository: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = commands::diff::handle_diff(&repo, args) {
        eprintln!("Diff failed: {}", e);
        std::process::exit(1);
    }
}

fn handle_stats(args: &[String]) {
    // Find the git repository
    let repo = match find_repository(&Vec::<String>::new()) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Failed to find repository: {}", e);
            std::process::exit(1);
        }
    };
    // Parse stats-specific arguments
    let mut json_output = false;
    let mut commit_sha = None;
    let mut commit_range: Option<CommitRange> = None;
    let mut ignore_patterns: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_output = true;
                i += 1;
            }
            "--ignore" => {
                // Collect all arguments after --ignore until we hit another flag or commit SHA
                // This supports shell glob expansion: `--ignore *.lock` expands to `--ignore Cargo.lock package.lock`
                i += 1;
                let mut found_pattern = false;
                while i < args.len() {
                    let arg = &args[i];
                    // Stop if we hit another flag
                    if arg.starts_with("--") {
                        break;
                    }
                    // Stop if this looks like a commit SHA or range (contains ..)
                    if arg.contains("..")
                        || (commit_sha.is_none() && !found_pattern && arg.len() >= 7)
                    {
                        // Could be a commit SHA, stop collecting patterns
                        break;
                    }
                    ignore_patterns.push(arg.clone());
                    found_pattern = true;
                    i += 1;
                }
                if !found_pattern {
                    eprintln!("--ignore requires at least one pattern argument");
                    std::process::exit(1);
                }
            }
            _ => {
                // First non-flag argument is treated as commit SHA or range
                if commit_sha.is_none() {
                    let arg = &args[i];
                    // Check if this is a commit range (contains "..")
                    if arg.contains("..") {
                        let parts: Vec<&str> = arg.split("..").collect();
                        if parts.len() == 2 {
                            match CommitRange::new_infer_refname(
                                &repo,
                                parts[0].to_string(),
                                parts[1].to_string(),
                                // @todo this is probably fine, but we might want to give users an option to override from this command.
                                None,
                            ) {
                                Ok(range) => {
                                    commit_range = Some(range);
                                }
                                Err(e) => {
                                    eprintln!("Failed to create commit range: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            eprintln!("Invalid commit range format. Expected: <commit>..<commit>");
                            std::process::exit(1);
                        }
                    } else {
                        commit_sha = Some(arg.clone());
                    }
                    i += 1;
                } else {
                    eprintln!("Unknown stats argument: {}", args[i]);
                    std::process::exit(1);
                }
            }
        }
    }

    // Handle commit range if detected
    if let Some(range) = commit_range {
        match range_authorship::range_authorship(range, true, &ignore_patterns) {
            Ok(stats) => {
                if json_output {
                    let json_str = serde_json::to_string(&stats).unwrap();
                    println!("{}", json_str);
                } else {
                    range_authorship::print_range_authorship_stats(&stats);
                }
            }
            Err(e) => {
                eprintln!("Range authorship failed: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if let Err(e) = stats_command(&repo, commit_sha.as_deref(), json_output, &ignore_patterns) {
        match e {
            crate::error::GitAiError::Generic(msg) if msg.starts_with("No commit found:") => {
                eprintln!("{}", msg);
            }
            _ => {
                eprintln!("Stats failed: {}", e);
            }
        }
        std::process::exit(1);
    }
}

fn get_all_files_for_mock_ai(working_dir: &str) -> Vec<String> {
    // Find the git repository
    let repo = match find_repository_in_path(&working_dir) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Failed to find repository: {}", e);
            return Vec::new();
        }
    };
    match repo.get_staged_and_unstaged_filenames() {
        Ok(filenames) => filenames.into_iter().collect(),
        Err(_) => Vec::new(),
    }
}

/// 自定义帮助命令：展示 git-ai 的核心概念和工作原理
fn handle_myhelp() {
    println!("════════════════════════════════════════════════════════════════");
    println!("             🤖 git-ai 核心概念与工作原理 🤖");
    println!("════════════════════════════════════════════════════════════════\n");

    println!("📚 什么是 git-ai？");
    println!("───────────────────────────────────────────────────────────────");
    println!("git-ai 是一个 Git 包装器，用于追踪代码的真实作者（AI 或人工）。");

    println!("🔄 核心工作流程");
    println!("───────────────────────────────────────────────────────────────");
    println!("1. 代码编写：你使用 AI 助手（如 Cursor、Copilot）编写代码");
    println!("2. 创建检查点：git-ai 记录这些代码是 AI 生成的");
    println!("3. 提交代码：使用 git commit，git-ai 自动追踪归属");
    println!("4. 查看归属：使用 git-ai blame 查看每行代码的作者\n");

    println!("🎯 关键概念");
    println!("───────────────────────────────────────────────────────────────");
    println!("• Checkpoint（检查点）");
    println!("  - 代码快照，记录某个时刻的代码归属");
    println!("  - 分为 Human（人工）和 AI（AI 生成）两种类型");
    println!("");
    println!("• Working Log（工作日志）");
    println!("  - 提交前的临时检查点集合");
    println!("  - 存储在 .git/ai/working_logs/ 目录");
    println!("");
    println!("• Authorship Log（归属日志）");
    println!("  - 提交后的永久归属记录");
    println!("  - 存储在 .git/ai/authorship/ 目录");
    println!("");
    println!("• Rewrite Log（重写日志）");
    println!("  - 记录 Git 历史重写事件（如 amend、rebase）");
    println!("  - 确保即使提交历史改变，归属信息仍然准确\n");

    println!("💡 常用命令");
    println!("───────────────────────────────────────────────────────────────");
    println!("git-ai checkpoint        创建检查点（通常自动触发）");
    println!("git-ai blame <file>      查看文件的代码归属");
    println!("git-ai stats [commit]    查看提交的 AI/人工代码统计");
    println!("git-ai diff <commit>     查看差异并标注归属");
    println!("git-ai show <commit>     显示提交的归属日志");
    println!("git-ai help              查看完整命令列表\n");

    println!("🌟 实际例子");
    println!("───────────────────────────────────────────────────────────────");
    println!("# 1. Cursor 生成代码后创建检查点");
    println!("$ git-ai checkpoint cursor");
    println!("");
    println!("# 2. 提交代码（git-ai 自动追踪）");
    println!("$ git commit -m \"feat: add login\"");
    println!("");
    println!("# 3. 查看代码归属");
    println!("$ git-ai blame src/login.rs");
    println!("abc123 (Cursor)  1) fn login() {{");
    println!("abc123 (Cursor)  2)     // AI 生成的代码");
    println!("def456 (Human)   3)     // 你手动修改的代码");
    println!("abc123 (Cursor)  4) }}\n");

    println!("🔗 更多信息");
    println!("───────────────────────────────────────────────────────────────");
    println!("文档: https://github.com/acunniffe/git-ai");
    println!("问题: https://github.com/acunniffe/git-ai/issues");
    println!("");
    println!("════════════════════════════════════════════════════════════════\n");

    std::process::exit(0);
}
