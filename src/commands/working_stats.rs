use crate::authorship::attribution_tracker::Attribution;
use crate::authorship::virtual_attribution::VirtualAttributions;
use crate::error::GitAiError;
use crate::git::find_repository;
use crate::git::repository::Repository;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingStats {
    pub files_changed: usize,
    pub pure_human_lines: u32,
    pub mixed_lines: u32,
    pub pure_ai_lines: u32,
    pub total_lines: u32,
    pub by_file: HashMap<String, FileStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    pub pure_human_lines: u32,
    pub mixed_lines: u32,
    pub pure_ai_lines: u32,
    pub total_lines: u32,
}

impl Default for WorkingStats {
    fn default() -> Self {
        Self {
            files_changed: 0,
            pure_human_lines: 0,
            mixed_lines: 0,
            pure_ai_lines: 0,
            total_lines: 0,
            by_file: HashMap::new(),
        }
    }
}

/// Calculate statistics from working log (checkpoint.jsonl only)
pub fn calculate_working_stats(
    repo: &Repository,
    ignore_patterns: &[String],
) -> Result<WorkingStats, GitAiError> {
    // Get current HEAD commit SHA
    let base_commit = match repo.head() {
        Ok(head) => match head.target() {
            Ok(oid) => oid,
            Err(_) => "initial".to_string(),
        },
        Err(_) => "initial".to_string(),
    };

    // Build VirtualAttributions from working log only
    let working_va = VirtualAttributions::from_just_working_log(
        repo.clone(),
        base_commit.clone(),
        None,
    )?;

    // Calculate statistics
    let mut stats = WorkingStats::default();

    for (file_path, (char_attrs, _line_attrs)) in &working_va.attributions {
        // Skip ignored files
        if should_ignore_file(file_path, ignore_patterns) {
            continue;
        }

        // Get file content from working directory
        let file_content = if let Ok(workdir) = repo.workdir() {
            let abs_path = workdir.join(file_path);
            if abs_path.exists() {
                std::fs::read_to_string(&abs_path).unwrap_or_default()
            } else {
                continue;
            }
        } else {
            continue;
        };

        if file_content.is_empty() {
            continue;
        }

        // Calculate stats for this file
        let file_stats = calculate_file_stats(&file_content, char_attrs)?;

        // Add to total
        stats.pure_human_lines += file_stats.pure_human_lines;
        stats.mixed_lines += file_stats.mixed_lines;
        stats.pure_ai_lines += file_stats.pure_ai_lines;
        stats.total_lines += file_stats.total_lines;
        stats.by_file.insert(file_path.to_string(), file_stats);
        stats.files_changed += 1;
    }

    Ok(stats)
}

/// Calculate statistics for a single file
fn calculate_file_stats(
    content: &str,
    attributions: &[Attribution],
) -> Result<FileStats, GitAiError> {
    let lines: Vec<&str> = content.lines().collect();

    // Track all authors for each line (not just the last one)
    // Vec of sets: line_authors[line_idx] = set of authors who touched this line
    let mut line_authors: Vec<std::collections::HashSet<String>> =
        vec![std::collections::HashSet::new(); lines.len()];

    // Build accurate line boundaries by scanning the actual content
    let mut line_boundaries: Vec<(usize, usize)> = Vec::new(); // (start, end) for each line
    let mut char_pos = 0;
    for line in &lines {
        let start = char_pos;
        let end = char_pos + line.len();
        line_boundaries.push((start, end));
        // Move to the next character after this line
        char_pos = end;
        // Skip the newline character(s)
        if char_pos < content.len() {
            let c = content.chars().nth(char_pos).unwrap();
            if c == '\r' {
                char_pos += 1;
                if char_pos < content.len() && content.chars().nth(char_pos).unwrap() == '\n' {
                    char_pos += 1;
                }
            } else if c == '\n' {
                char_pos += 1;
            }
        }
    }

    // Debug: print basic info
    eprintln!("DEBUG: content has {} lines", lines.len());
    eprintln!("DEBUG: content.len() = {}", content.len());
    eprintln!("DEBUG: {} attributions", attributions.len());

    // Debug: print each line with accurate character positions
    for (i, line) in lines.iter().enumerate() {
        let (start, end) = line_boundaries.get(i).copied().unwrap_or((0, 0));
        eprintln!("DEBUG: line {} (char {}-{}, len={}): {:?}",
                  i, start, end - 1, end - start, line);
    }

    // Mark each line with all its authors (in order)
    for (attr_idx, attr) in attributions.iter().enumerate() {
        let start_char = attr.start;
        let end_char = attr.end.min(content.len());

        eprintln!("DEBUG: attr[{}]: start={}, end={}, author={}",
                  attr_idx, start_char, end_char, attr.author_id);

        // Find which lines this attribution covers
        for (line_idx, &(line_start, line_end)) in line_boundaries.iter().enumerate() {
            // Check if this attribution overlaps this line
            let overlaps = !(end_char <= line_start || start_char >= line_end);

            if overlaps {
                eprintln!("DEBUG:   line {} (char {}-{}) overlaps with attr ({}-{})",
                          line_idx, line_start, line_end - 1, start_char, end_char - 1);

                // Add this author to the line's author set
                line_authors[line_idx].insert(attr.author_id.clone());
                eprintln!("DEBUG:   line {} now has {} authors: {:?}",
                          line_idx, line_authors[line_idx].len(),
                          line_authors[line_idx]);
            }
        }
    }

    // Categorize lines based on author history
    let mut pure_human_lines = 0;
    let mut pure_ai_lines = 0;
    let mut mixed_lines = 0;
    let mut total_lines = 0;

    for (line_idx, authors) in line_authors.iter().enumerate() {
        if authors.is_empty() {
            // No attribution at all = skip this line
            eprintln!("DEBUG: line {} ({:?}) -> no authors -> skipping",
                      line_idx, lines.get(line_idx));
            continue;
        } else if authors.len() == 1 {
            // Only one author
            let author = authors.iter().next().unwrap();
            if author == "human" {
                pure_human_lines += 1;
                total_lines += 1;
                eprintln!("DEBUG: line {} ({:?}) -> single author: human",
                          line_idx, lines.get(line_idx));
            } else {
                pure_ai_lines += 1;
                total_lines += 1;
                eprintln!("DEBUG: line {} ({:?}) -> single author: ai ({})",
                          line_idx, lines.get(line_idx), author);
            }
        } else {
            // Multiple authors
            if authors.contains("human") {
                // Human + AI(s) = mixed
                mixed_lines += 1;
                total_lines += 1;
                eprintln!("DEBUG: line {} ({:?}) -> human + AI -> mixed",
                          line_idx, lines.get(line_idx));
            } else {
                // AI + AI = pure_ai (multiple AI sessions still count as pure AI)
                pure_ai_lines += 1;
                total_lines += 1;
                eprintln!("DEBUG: line {} ({:?}) -> multiple AI sessions -> pure_ai",
                          line_idx, lines.get(line_idx));
            }
        }
    }

    eprintln!("DEBUG: final: human={}, ai={}, mixed={}, total={}",
              pure_human_lines, pure_ai_lines, mixed_lines, total_lines);

    Ok(FileStats {
        pure_human_lines,
        mixed_lines,
        pure_ai_lines,
        total_lines,
    })
}

/// Check if a file should be ignored based on patterns
fn should_ignore_file(file_path: &str, ignore_patterns: &[String]) -> bool {
    for pattern in ignore_patterns {
        if file_path.contains(pattern) || glob_match(file_path, pattern) {
            return true;
        }
    }
    false
}

/// Simple glob matching (supports * wildcard and simple patterns)
fn glob_match(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return text == pattern;
    }

    // Split by wildcard and match
    let parts: Vec<&str> = pattern.split('*').collect();

    // Pattern: *.txt
    if parts.len() == 2 && parts[0].is_empty() {
        return text.ends_with(parts[1]);
    }

    // Pattern: prefix*
    if parts.len() == 2 && parts[1].is_empty() {
        return text.starts_with(parts[0]);
    }

    // Pattern: *middle*
    if parts.len() == 2 {
        return text.contains(parts[1]);
    }

    // Pattern: prefix*suffix
    if parts.len() == 3 && parts[1].is_empty() {
        return text.starts_with(parts[0]) && text.ends_with(parts[2]);
    }

    false
}

/// Print working stats to terminal
pub fn print_working_stats(stats: &WorkingStats) {
    println!("\nWorking Area Stats (uncommitted changes)");
    println!("════════════════════════════════════════\n");
    println!("Files changed: {}\n", stats.files_changed);

    if stats.total_lines == 0 {
        println!("No changes detected in working area.");
        return;
    }

    // Calculate percentages
    let human_pct = (stats.pure_human_lines as f64 / stats.total_lines as f64) * 100.0;
    let mixed_pct = (stats.mixed_lines as f64 / stats.total_lines as f64) * 100.0;
    let ai_pct = (stats.pure_ai_lines as f64 / stats.total_lines as f64) * 100.0;

    // Draw progress bar
    let bar_width = 40;
    let human_bars = ((human_pct / 100.0) * bar_width as f64) as usize;
    let mixed_bars = ((mixed_pct / 100.0) * bar_width as f64) as usize;
    let ai_bars = bar_width - human_bars - mixed_bars;

    println!(
        "  you  {}{}{}{}",
        "█".repeat(human_bars),
        "▒".repeat(mixed_bars),
        "░".repeat(ai_bars),
        " ai"
    );

    println!(
        "     {:>3}{:>12}mixed {:>3}%{:>12}{:>3}%",
        format!("{:.0}%", human_pct),
        "",
        mixed_pct,
        "",
        ai_pct
    );
    println!();

    println!("Summary:");
    println!("  Pure human:   {} lines", stats.pure_human_lines);
    println!("  Mixed (AI+human): {} lines", stats.mixed_lines);
    println!("  Pure AI:      {} lines", stats.pure_ai_lines);
    println!("  Total:        {} lines", stats.total_lines);

    // Print per-file breakdown
    if !stats.by_file.is_empty() {
        println!("\nBy file:");
        let mut files: Vec<_> = stats.by_file.iter().collect();
        files.sort_by(|a, b| b.1.total_lines.cmp(&a.1.total_lines));

        for (file, file_stats) in files {
            if file_stats.total_lines > 0 {
                println!(
                    "  {:30}: {} human, {} mixed, {} ai",
                    file,
                    file_stats.pure_human_lines,
                    file_stats.mixed_lines,
                    file_stats.pure_ai_lines
                );
            }
        }
    }
}

pub fn handle_working_stats(args: &[String]) -> Result<(), GitAiError> {
    // Find repository
    let repo = match find_repository(&Vec::new()) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Failed to find repository: {}", e);
            std::process::exit(1);
        }
    };

    // Parse arguments
    let mut json_output = false;
    let mut ignore_patterns: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json_output = true;
                i += 1;
            }
            "--ignore" => {
                i += 1;
                if i < args.len() && !args[i].starts_with("--") {
                    ignore_patterns.push(args[i].clone());
                    i += 1;
                }
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    // Calculate stats
    let stats = calculate_working_stats(&repo, &ignore_patterns)?;

    // Output
    if json_output {
        let json_str = serde_json::to_string_pretty(&stats).unwrap();
        println!("{}", json_str);
    } else {
        print_working_stats(&stats);
    }

    Ok(())
}
