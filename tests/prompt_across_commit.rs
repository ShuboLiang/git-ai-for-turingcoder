#[macro_use]
mod repos;
mod test_utils;

use repos::test_file::ExpectedLineExt;
use repos::test_repo::TestRepo;
use git_ai::authorship::authorship_log::LineRange;

#[test]
fn test_change_across_commits() {
    let repo = TestRepo::new();
    let mut file = repo.filename("foo.py");

    file.set_contents(lines![
        "def print_name(name: str) -> None:".ai(),
        "    \"\"\"Print the given name.\"\"\"".ai(),
        "    if name == 'foobar':".ai(),
        "        print('name not allowed!')".ai(),
        "    print(f\"Hello, {name}!\")".ai(),
        "".ai(),
        "print_name(\"Michael\")".ai(),
    ]);
    println!("file: {}", file.lines.iter().map(|line| line.contents.clone()).collect::<Vec<String>>().join("\n"));

    let commit = repo.stage_all_and_commit("Initial all AI").unwrap();
    // commit.print_authorship();
    let initial_ai_entry = commit.authorship_log.attestations.first().unwrap().entries.first().unwrap();
    println!("initial_ai_entry: {:?}", initial_ai_entry);

    file.replace_at(4, "    print(f\"Hello, {name.upper()}!\")".ai());
    println!("file: {}", file.lines.iter().map(|line| line.contents.clone()).collect::<Vec<String>>().join("\n"));
    file.insert_at(4, lines!["    name = name.upper()".human()]);
    println!("file: {}", file.lines.iter().map(|line| line.contents.clone()).collect::<Vec<String>>().join("\n"));

    let commit = repo.stage_all_and_commit("add more AI").unwrap();
    // commit.print_authorship();

    let file_attestation = commit.authorship_log.attestations.first().unwrap();
    println!("file_attestation: {:?}", file_attestation);
    assert_eq!(file_attestation.entries.len(), 1);
    let second_ai_prompt_hash = commit.authorship_log.metadata.prompts.keys().next().unwrap();
    println!("second_ai_prompt_hash: {}", second_ai_prompt_hash);
    assert_ne!(*second_ai_prompt_hash, initial_ai_entry.hash);
    let second_ai_entry = file_attestation.entries.first().unwrap();
    println!("second_ai_entry: {:?}", second_ai_entry);
    assert_eq!(second_ai_entry.line_ranges, vec![LineRange::Single(6)]);
    // This is failing, because for some reason, even though the edit is coming from a new AI prompt,
    // and we confirmed that the new AI prompt is showing up properly in the prompts section,
    // we are crediting the old prompt for the edit.
    assert_ne!(second_ai_entry.hash, initial_ai_entry.hash);
}
