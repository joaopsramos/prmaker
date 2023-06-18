mod inspect;
mod pr;

use colored::Colorize;
use octocrab::{models::User, OctocrabBuilder, Page};
use std::{
    env,
    fmt::Display,
    io::{self, Write},
    process::exit,
};

const GITHUB_TOKEN_VAR: &str = "GITHUB_TOKEN";
const GITHUB_USER_VAR: &str = "GITHUB_USER";

#[tokio::main]
async fn main() {
    let user = get_user();
    let token = get_token();

    let mut pr = pr::PR::build();

    println!("\n{}", "** Review PR **".blue());
    println!("{pr}");

    proceed_question();

    let octocrab = OctocrabBuilder::new()
        .personal_token(token)
        .build()
        .unwrap();

    println!("\nCreating PR...");

    if pr.create(&octocrab).await.is_err() {
        exit(1)
    }

    println!("\nAssigning to you...");

    pr.assign_self(&octocrab, &user).await;

    let collaborators_resp = octocrab
        .orgs(&pr.base)
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
                    .pulls(&pr.base, &pr.repo)
                    .request_reviews(pr.number.unwrap(), usernames, [])
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

    println!("\nPR: {}", pr.link.unwrap())
}

fn get_user() -> String {
    env::var(GITHUB_USER_VAR).unwrap_or_else(|_err| {
        println!(
            "{}",
            format!("Couldn't get {} environment variable", GITHUB_USER_VAR).red()
        );
        exit(1);
    })
}

fn get_token() -> String {
    env::var(GITHUB_TOKEN_VAR).unwrap_or_else(|_err| {
        println!(
            "{}",
            format!("Couldn't get {} environment variable", GITHUB_TOKEN_VAR).red()
        );
        println!("Please ensure the variable is available and it is a valid token");
        exit(1);
    })
}

fn proceed_question() {
    print!("\n{}", "Proceed? (y/n): ".yellow());
    flush_line();

    loop {
        let mut proceed_opt = String::new();
        io::stdin().read_line(&mut proceed_opt).unwrap();

        match proceed_opt.trim() {
            "y" => break,
            "n" => {
                println!("\nClosing...");
                exit(0);
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
                    println!("{}", "Reviewer not found".red())
                }
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

fn flush_line() {
    io::stdout().flush().unwrap();
}
