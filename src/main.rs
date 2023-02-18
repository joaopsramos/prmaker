use colored::Colorize;
use octocrab::{
    models::{pulls::PullRequest, User},
    Error::GitHub,
    OctocrabBuilder, Page,
};
use regex::Regex;
use std::{
    env,
    fmt::Display,
    io::{self, Write},
    process::Command,
};

const GITHUB_TOKEN_VAR: &str = "GITHUB_TOKEN";
const GITHUB_USER_VAR: &str = "GITHUB_USER";
const YT_ISSUE_REGEX: &str = r"^\w+/(\w+-\d+)";
const BASE_REGEX: &str = r":([\w-]+)/";
const REPO_REGEX: &str = r"/([\w-]+).git";

#[tokio::main]
async fn main() {
    let user = env::var(GITHUB_USER_VAR).unwrap_or_else(|_err| {
        println!(
            "{}",
            format!("Couldn't get {} environment variable", GITHUB_USER_VAR).red()
        );
        exit_with_code(1);
    });
    let token = match env::var(GITHUB_TOKEN_VAR) {
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

    let remote_url = get_remote_url();
    let base = get_base(&remote_url);
    let repo = get_repo(&remote_url);

    print!("\n");

    let current_branch = get_current_branch();
    let pr_title = get_pr_title();
    let yt_issue = match get_yt_issue_from_branch_name(&current_branch) {
        Some(yt_issue) => yt_issue,
        None => {
            println!(
                "\n{}",
                "Couldn't get Youtrack issue from branch name. Please provide one or leave it empty".red()
            );
            request_yt_issue()
        }
    };
    let pr_body = get_pr_body();
    let full_pr_body = build_full_pr_body(&pr_body, &yt_issue);

    println!("\n{}", "** Review PR **".blue());
    println!("Title: {}", pr_title.cyan());
    println!("Body: {}", pr_body.cyan());
    println!("Youtrack issue: {}", yt_issue.cyan());
    println!("Remote branch: {}", current_branch.cyan());
    println!("Remote: {}", format!("{}/{}", base, repo).cyan());

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

    println!("\nCreating PR...");

    let octocrab = OctocrabBuilder::new()
        .personal_token(token)
        .build()
        .unwrap();

    let pr_resp = octocrab
        .pulls(&base, &repo)
        .create(pr_title, &current_branch, "next")
        .body(full_pr_body)
        .send()
        .await;

    let pr = match pr_resp {
        Ok(pr) => {
            let pr_link = get_pr_link(&pr);

            print!("\n{}", "PR created successfully: ".green());
            println!("{}", pr_link);

            pr
        }
        Err(GitHub { source, .. }) => {
            println!("\n{}", "Something went wrong, error message: ".red());
            println!("{source}");
            exit_with_code(1);
        }
        err => panic!("{:?}", err),
    };

    println!("\nAssigning to you...");

    let assign_resp = octocrab
        .issues(&base, &repo)
        .add_assignees(pr.number, &[&user])
        .await;

    match assign_resp {
        Ok(_) => println!("\n{}", "Assigned successfully".green()),
        Err(_) => println!("\n{}", "Error when assigning".red()),
    }

    let collaborators_resp = octocrab
        .orgs(&base)
        .list_members()
        .per_page(100)
        .send()
        .await;

    match collaborators_resp {
        Ok(collaborators) => {
            let reviewers = get_selected_reviewers(collaborators);
            let usernames: Vec<String> = reviewers.iter().map(|r| r.username.clone()).collect();

            if reviewers.is_empty() {
                println!("\nNo reviewers to request");
            } else {
                let assigness_resp = octocrab
                    .pulls(&base, &repo)
                    .request_reviews(pr.number, usernames, [])
                    .await;

                match assigness_resp {
                    Ok(_) => println!("\n{}", "Reviewers requested successfully".green()),
                    Err(_) => println!("{}", "Failed to request reviewers".red()),
                }
            }
        }
        Err(_) => {
            println!("\n{}", "Error fetching collaborators, ignoring...".red());
        }
    }

    println!("\nPR: {}", get_pr_link(&pr))
}

fn get_current_branch() -> String {
    let stdout = Command::new("git")
        .arg("branch")
        .arg("--show-current")
        .output()
        .expect("failed to run `git branch --show-current`")
        .stdout;

    let current_branch = String::from_utf8(stdout).unwrap();

    current_branch.trim().to_owned()
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

    current_branch.trim().to_owned()
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

fn get_remote_url() -> String {
    let stdout = Command::new("git")
        .arg("config")
        .arg("--get")
        .arg("remote.origin.url")
        .output()
        .expect("failed to run `git config --get remote.origin.url`")
        .stdout;

    let remote_url = String::from_utf8(stdout).unwrap();

    remote_url.trim().to_owned()
}

fn get_base(remote_url: &str) -> String {
    let err = format!(
        "Failed to get the user/org name from remote url: {}",
        remote_url
    );
    let re = Regex::new(BASE_REGEX).unwrap();

    re.captures(remote_url)
        .expect(&err)
        .get(1)
        .expect(&err)
        .as_str()
        .to_owned()
}

fn get_repo(remote_url: &str) -> String {
    let err = format!(
        "Failed to get the repo name from remote url: {}",
        remote_url
    );
    let re = Regex::new(REPO_REGEX).unwrap();

    re.captures(remote_url)
        .expect(&err)
        .get(1)
        .expect(&err)
        .as_str()
        .to_owned()
}

#[derive(Debug, Clone)]
struct Reviewer {
    username: String,
    index: usize,
    selected: bool,
}

impl Display for Reviewer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let result = format!("{} - {}", self.index.to_string().purple(), self.username);

        if self.selected {
            write!(f, "{}", result.cyan())
        } else {
            write!(f, "{}", result)
        }
    }
}

fn get_reviewers(collaborators: Page<User>) -> Vec<Reviewer> {
    collaborators
        .into_iter()
        .enumerate()
        .map(|(index, user)| Reviewer {
            username: user.login,
            index,
            selected: false,
        })
        .collect()
}

fn get_selected_reviewers(collaborators: Page<User>) -> Vec<Reviewer> {
    let mut reviewers = get_reviewers(collaborators);
    loop {
        let mut opt = String::new();

        println!("\n{}", "** Reviewers **".blue());

        for reviewer in &reviewers {
            println!("{}", reviewer);
        }

        print!("\n{}", "Add a reviewer (empty to proceed): ".yellow());
        flush_line();

        io::stdin().read_line(&mut opt).unwrap();

        if opt.trim().is_empty() {
            break;
        }

        match opt.trim().parse::<usize>() {
            Ok(index) => {
                if let Some(reviewer) = reviewers.iter_mut().find(|r| r.index == index) {
                    reviewer.selected = !reviewer.selected
                } else {
                    println!("{}", "Reviewer not found".red());
                }
                ()
            }
            Err(_) => println!("{}", "Invalid option, it must be a valid number".red()),
        }
    }

    reviewers
        .clone()
        .into_iter()
        .filter(|r| r.selected)
        .collect()
}

fn get_pr_link(pr: &PullRequest) -> String {
    let html_url = pr.html_url.as_ref().unwrap();

    format!(
        "{}://{}{}",
        html_url.scheme(),
        html_url.host().unwrap(),
        html_url.path()
    )
}

fn exit_with_code(code: i32) -> ! {
    std::process::exit(code)
}

fn flush_line() {
    io::stdout().flush().unwrap();
}
