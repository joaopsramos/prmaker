use colored::Colorize;
use octocrab::{Error::GitHub, GitHubError, OctocrabBuilder};
use regex::Regex;
use std::{
    io::{self, Write},
    process::Command,
};

const GITHUB_TOKEN_VAR: &str = "GITHUB_TOKEN";
const YT_ISSUE_REGEX: &str = r"^\w+/(\w+-\d+)";

#[tokio::main]
async fn main() {
    let token = match std::env::var(GITHUB_TOKEN_VAR) {
        Ok(token) => token,
        Err(_) => {
            println!(
                "{}",
                format!("Couldn't get {} environment variable", GITHUB_TOKEN_VAR).red()
            );
            println!("Please ensure the variable is available and it is a valid token");
            exit_with_code(1);
        }
    };

    print!("\n");

    let current_branch = get_current_branch();
    let pr_title = get_pr_title();
    let yt_issue = match get_yt_issue_from_branch_name(&current_branch) {
        Some(yt_issue) => yt_issue,
        None => {
            println!(
                "\n{} Please provide one or leave it empty",
                "Couldn't get Youtrack issue from branch name.".red()
            );
            request_yt_issue()
        }
    };
    let pr_body = get_pr_body();
    let full_pr_body = build_full_pr_body(&pr_body, &yt_issue);

    println!("\n{}", "** Review PR **".blue());
    println!("Remote branch: {}", current_branch.cyan());
    println!("Title: {}", pr_title.cyan());
    println!("Body: {}", pr_body.cyan());
    println!("Youtrack issue: {}", yt_issue.cyan());

    print!("\n{}", "Proceed? (y/n): ".yellow());
    flush_line();

    loop {
        let mut proceed_opt = String::new();
        io::stdin().read_line(&mut proceed_opt).unwrap();

        match proceed_opt.trim() {
            "y" => break,
            "n" => {
                println!("\nClosing...");
                exit_with_code(0);
            }
            _ => {
                println!(
                    "Please digit {} for {} and {} for {}",
                    "y".green(),
                    "yes".green(),
                    "n".red(),
                    "no".red()
                );

                continue;
            }
        }
    }

    println!("\n{}", "Creating PR...".green());

    let octocrab = OctocrabBuilder::new()
        .personal_token(token)
        .build()
        .unwrap();

    let pr_response = octocrab
        .pulls("joaopsramos", "testes")
        .create(pr_title, current_branch, "master")
        .body(full_pr_body)
        .send()
        .await;

    match pr_response {
        Ok(pr) => println!("\n{}", "PR created successfully".green()),
        Err(GitHub { source, .. }) => {
            println!("\n{}", "Something went wrong, error message: ".red());
            println!("{source}");
            exit_with_code(1);
        }
        err => panic!("{:?}", err),
    }
}

fn get_current_branch() -> String {
    let stdout = Command::new("git")
        .arg("branch")
        .arg("--show-current")
        .output()
        .expect("failed to run `git branch --show-current`")
        .stdout;

    let current_branch = String::from_utf8(stdout).unwrap();

    current_branch
        .strip_suffix("\n")
        .unwrap_or(&current_branch)
        .to_owned()
}

fn get_yt_issue_from_branch_name(branch: &str) -> Option<String> {
    let re = Regex::new(YT_ISSUE_REGEX).unwrap();
    let issue = re.captures(branch)?.get(1)?.as_str();

    Some(issue.to_owned())
}

fn request_yt_issue() -> String {
    let mut issue = String::new();

    print!("Youtrack issue: ");
    flush_line();

    io::stdin().read_line(&mut issue).unwrap();

    issue.trim().to_owned()
}

fn get_pr_title() -> String {
    let last_commit = get_last_commit();
    let mut pr_title = String::new();

    println!("PR title: {}", last_commit.purple());
    print!("Leave it blank to use the title above or digit a new one: ");
    flush_line();

    io::stdin().read_line(&mut pr_title).unwrap();

    if pr_title.trim().is_empty() {
        last_commit.to_string()
    } else {
        pr_title.trim().to_owned()
    }
}

fn get_last_commit() -> String {
    let stdout = Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--pretty=format:%s")
        .output()
        .expect("failed to run `git log -1 --pretty=%s`")
        .stdout;

    let current_branch = String::from_utf8(stdout).unwrap();

    current_branch
        .strip_suffix("\n")
        .unwrap_or(&current_branch)
        .trim()
        .to_owned()
}

fn get_pr_body() -> String {
    let default_body = "Title".to_owned();
    let mut pr_body = String::new();

    println!("\nPR body: {}", default_body.purple());
    print!("Leave it blank to use the body above or digit a new one: ");
    flush_line();

    io::stdin().read_line(&mut pr_body).unwrap();

    if pr_body.trim().is_empty() {
        default_body
    } else {
        pr_body.trim().to_owned()
    }
}
fn build_full_pr_body(title: &str, yt_issue: &str) -> String {
    format!("\
### What does this PR do?

{}

<!--
Please include a summary of the change and/or which issue is fixed. Please also include relevant motivation and context. List any dependencies that are required for this change, also provide (if appropriate) any evidence - screenshots, gifs, logs, etc.

Oh, remember to follow conventional commits (https://conventionalcommits.org) on pull request title ;)
-->

---

**Related issue:** {}
", title, yt_issue)
}

fn exit_with_code(code: i32) -> ! {
    std::process::exit(code)
}

fn flush_line() {
    io::stdout().flush().unwrap();
}
