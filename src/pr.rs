use colored::Colorize;
use octocrab::{models::pulls::PullRequest, Error::GitHub, Octocrab};
use regex::Regex;
use std::{fmt::Display, io, process::Command};

use crate::flush_line;

const YT_ISSUE_REGEX: &str = r"^\w+/(\w+-\d+)";
const BASE_REGEX: &str = r":([\w-]+)/";
const REPO_REGEX: &str = r"/([\w-]+)(.git)?$";

pub struct PR {
    pub branch: String,
    pub title: String,
    pub yt_issue: String,
    pub body: String,
    pub full_body: String,
    pub link: Option<String>,
    pub number: Option<u64>,
    pub base: String,
    pub repo: String,
}

impl PR {
    pub fn build() -> Self {
        let remote_url = get_remote_url();
        let base = get_base(&remote_url);
        let repo = get_repo(&remote_url);

        print!("\n");

        let current_branch = get_current_branch();
        let title = get_pr_title();
        let yt_issue = get_yt_issue(&current_branch);
        let body = get_pr_body();
        let full_body = build_full_pr_body(&body, &yt_issue);

        PR {
            branch: current_branch,
            title,
            yt_issue,
            body,
            full_body,
            base,
            repo,
            link: None,
            number: None,
        }
    }

    pub async fn create(&mut self, octocrab: &Octocrab) -> Result<(), ()> {
        let pr_resp = octocrab
            .pulls(&self.base, &self.repo)
            .create(&self.title, &self.branch, "next")
            .body(&self.full_body)
            .send()
            .await;

        match pr_resp {
            Ok(github_pr) => {
                self.number = Some(github_pr.number);
                self.link = Some(get_pr_link(&github_pr));

                print!("\n{}", "PR created successfully: ".green());
                println!("{}", self.link.as_ref().unwrap());

                Ok(())
            }
            Err(GitHub { source, .. }) => {
                println!("\n{}", "Something went wrong, error message: ".red());
                println!("{source}");

                Err(())
            }
            err => panic!("{:?}", err),
        }
    }

    pub async fn assign_self(&self, octocrab: &Octocrab, user: &str) {
        let assign_resp = octocrab
            .issues(&self.base, &self.repo)
            .add_assignees(self.number.unwrap(), &[&user])
            .await;

        match assign_resp {
            Ok(_) => println!("\n{}", "Assigned successfully".green()),
            Err(_) => println!("\n{}", "Error when assigning".red()),
        }
    }
}

impl Display for PR {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let result = format!(
            "\
Title: {}
Body: {}
Youtrack issue: {}
Remote branch: {}
Remote: {}",
            self.title.cyan(),
            self.body.cyan(),
            self.yt_issue.cyan(),
            self.branch.cyan(),
            format!("{}/{}", self.base, self.repo).cyan()
        );

        write!(f, "{}", result)
    }
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

fn get_yt_issue(branch: &str) -> String {
    match get_yt_issue_from_branch_name(&branch) {
        Some(yt_issue) => yt_issue,
        None => {
            println!(
                "\n{}",
                "Couldn't get Youtrack issue from branch name. Please provide one or leave it empty".red()
            );
            request_yt_issue()
        }
    }
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

fn get_pr_link(pr: &PullRequest) -> String {
    let html_url = pr.html_url.as_ref().unwrap();

    format!(
        "{}://{}{}",
        html_url.scheme(),
        html_url.host().unwrap(),
        html_url.path()
    )
}
