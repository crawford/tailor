// Copyright 2017 CoreOS, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use checks::Check;
use checks::max_body_line_length::MaxBodyLineLength;
use checks::max_summary_length::MaxSummaryLength;
use checks::no_capitalize_summary::NoCapitalizeSummary;
use checks::summary_scope::SummaryScope;
use checks::requires_body::RequiresBody;
use checks::no_wip::NoWip;
use checks::no_fixup::NoFixup;
use checks::no_squash::NoSquash;
use config;
use errors::*;
use github_rs::client::Github;
use serde_json::{self, Value};
use std::sync::mpsc::Sender;
use worker;

#[derive(Serialize, Deserialize)]
struct Comment {
    body: String,
    user: CommentUser,
}

#[derive(Serialize, Deserialize)]
struct CommentUser {
    login: String,
}

#[derive(Serialize, Deserialize)]
struct Collaborator {
    permission: String,
}

#[derive(Serialize, Deserialize)]
struct Commit {
    commit: Commit2,
    sha: String,
}

#[derive(Serialize, Deserialize)]
struct Commit2 {
    message: String,
}

pub struct CommitData {
    pub summary: String,
    pub body: Vec<String>,
}

pub fn run_checks(
    job_struct: &worker::PullRequestJob,
    tx: &Sender<worker::Job>,
    config: &config::Config,
) -> Result<()> {
    let owner = &job_struct.owner;
    let repo = &job_struct.repo;
    let number = job_struct.number;
    let repo_config = match config.repos.iter().find(|curr_repo| {
        owner == &curr_repo.owner && repo == &curr_repo.repo
    }) {
        Some(repo_config) => repo_config,
        None => {
            return Err(
                format!("Could not find repo {}/{} in the config", owner, repo).into(),
            );
        }
    };

    let client = Github::new(&config.access_token).chain_err(
        || "Failed to create new Github client",
    )?;

    let commits = client
        .get()
        .repos()
        .owner(owner)
        .repo(repo)
        .pulls()
        .number(number.to_string().as_str())
        .commits()
        .execute();

    let commits_json: Value;
    match commits {
        Ok((_, _, Some(json))) => {
            commits_json = json;
        }
        Ok((_, _, None)) => {
            return Err(
                "Could not get PR commit data (it is possible that this was trigger by an
                issue comment not related to a PR)!"
                    .into(),
            );
        }
        Err(err) => {
            return Err(err.into());
        }
    }

    let commits_arr: Vec<Commit> = serde_json::from_value(commits_json).chain_err(|| {
        "Failed to deserialize PR JSON (it is possible that this was trigger by an issue comment
        not related to a PR)"
    })?;
    // We want to fail the last commit to let github know that the PR is bad. Otherwise, github
    // will only look at the last commit, regardless if previous commits failed
    let mut commit_failed = false;
    let mut last_commit: Option<Commit> = None;
    // We don't need to change the status of the last commit if it already failed
    let mut last_commit_failed = false;

    // We have to bind these variables to let so they don't go out of scope
    let mbll = MaxBodyLineLength;
    let msl = MaxSummaryLength;
    let ncs = NoCapitalizeSummary;
    let ss = SummaryScope;
    let rb = RequiresBody;
    let nw = NoWip;
    let nf = NoFixup;
    let ns = NoSquash;
    let mut checks_vec: Vec<&Check> = Vec::new();
    checks_vec.push(&mbll);
    checks_vec.push(&msl);
    checks_vec.push(&ncs);
    checks_vec.push(&ss);
    checks_vec.push(&rb);
    checks_vec.push(&nw);
    checks_vec.push(&nf);
    checks_vec.push(&ns);

    let comments = client
        .get()
        .repos()
        .owner(owner)
        .repo(repo)
        .issues()
        .number(number.to_string().as_str())
        .comments()
        .execute();

    let comments_json: Value;
    match comments {
        Ok((_, _, Some(json))) => {
            comments_json = json;
        }
        Ok((_, _, None)) => {
            return Err("Could not get PR commit data!".into());
        }
        Err(err) => {
            return Err(err.into());
        }
    }
    let comments_arr: Vec<Comment> = serde_json::from_value(comments_json)?;
    for comment in comments_arr {
        if (&(comment.body)).starts_with("tailor disable") {
            let mut split = comment.body.as_str().split("tailor disable");
            // First item from split will be an empty string
            split.next();
            let disabled_check = split.next();
            if let Some(disabled_check_untrimmed) = disabled_check {
                let disabled_check = disabled_check_untrimmed.trim();
                let collab = client
                    .get()
                    .repos()
                    .owner(owner)
                    .repo(repo)
                    .collaborators()
                    .username(comment.user.login.as_str())
                    .permission()
                    .execute();
                let collaborator: Collaborator = match collab {
                    Ok((_, _, Some(json))) => serde_json::from_value(json)?,
                    Ok((_, _, None)) => {
                        println!("Could not get Collaborator data. User might not be collaborator");
                        continue;
                    }
                    Err(err) => {
                        return Err(err.into());
                    }
                };
                if collaborator.permission == "admin" {
                    let mut remove_index: Option<usize> = None;
                    {
                        if disabled_check == "all" {
                            checks_vec.clear();
                            break;
                        }
                        for (i, check) in checks_vec.iter().enumerate() {
                            if check.name() == disabled_check {
                                remove_index = Some(i);
                                break;
                            }
                        }
                    }
                    if let Some(remove_index) = remove_index {
                        checks_vec.remove(remove_index);
                    }
                }
            }
        }
    }


    for commit in commits_arr {
        let mut err_vec = Vec::new();
        let mut lines = commit.commit.message.lines();
        last_commit = Some(Commit {
            sha: commit.sha.clone(),
            commit: Commit2 { message: commit.commit.message.clone() },
        });
        let commit_summary = match lines.next() {
            None => {
                commit_failed = true;
                last_commit_failed = true;
                if let Err(err) = tx.send(worker::Job::Status(worker::StatusJob {
                    status: worker::Status {
                        state: worker::State::Failure,
                        description: "Commit has no message".to_string(),
                        context: "tailor".to_string(),
                    },
                    commit: worker::Commit {
                        owner: owner.clone(),
                        repo: repo.clone(),
                        sha: commit.sha.clone(),
                    },
                }))
                {
                    eprintln!("Failed to send check status to status thread: {}", err);
                }
                continue;
            }
            Some(summary) => summary.to_string(),
        };
        let empty_line = lines.next();
        let mut commit_body: Vec<String> = Vec::new();
        if let Some(empty_line) = empty_line {
            if empty_line != "" {
                commit_failed = true;
                last_commit_failed = true;
                if let Err(err) = tx.send(worker::Job::Status(worker::StatusJob {
                    status: worker::Status {
                        state: worker::State::Failure,
                        description: "Failed to parse commit due to
                                malformed commit message (text on second line)"
                            .to_string(),
                        context: "tailor".to_string(),
                    },
                    commit: worker::Commit {
                        owner: owner.clone(),
                        repo: repo.clone(),
                        sha: commit.sha.clone(),
                    },
                }))
                {
                    eprintln!("Failed to send check status to status thread: {}", err);
                }
                continue;
            }
            for line in lines {
                commit_body.push(line.to_string());
            }
        };

        let commit_data = CommitData {
            summary: commit_summary.clone(),
            body: commit_body.clone(),
        };

        for check in &checks_vec {
            if let Err(err) = check.verify(&repo_config.checks, &commit_data) {
                err_vec.push(err.to_string());
            }
        }

        if !err_vec.is_empty() {
            commit_failed = true;
            last_commit_failed = true;
            if let Err(err) = tx.send(worker::Job::Status(worker::StatusJob {
                status: worker::Status {
                    state: worker::State::Failure,
                    description: err_vec.join("\n"),
                    context: "tailor".to_string(),
                },
                commit: worker::Commit {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    sha: commit.sha.clone(),
                },
            }))
            {
                eprintln!("Failed to send check status to status thread: {}", err);
            }
        } else {
            last_commit_failed = false;
            if let Err(err) = tx.send(worker::Job::Status(worker::StatusJob {
                status: worker::Status {
                    state: worker::State::Success,
                    description: "All checks passed".to_string(),
                    context: "tailor".to_string(),
                },
                commit: worker::Commit {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    sha: commit.sha.clone(),
                },
            }))
            {
                eprintln!("Failed to send check status to status thread: {}", err);
            }
        }
    }
    if commit_failed && !last_commit_failed {
        if let Some(last_commit) = last_commit {
            if let Err(err) = tx.send(worker::Job::Status(worker::StatusJob {
                status: worker::Status {
                    state: worker::State::Failure,
                    description: "A previous commit failed".to_string(),
                    context: "tailor".to_string(),
                },
                commit: worker::Commit {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    sha: last_commit.sha.clone(),
                },
            }))
            {
                eprintln!("Failed to send check status to status thread: {}", err);
            }
        } else {
            eprintln!("Failed to mark last commmit as failed; last_commit == None");
        }
    }
    Ok(())
}
