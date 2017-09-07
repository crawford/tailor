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

use config::Config;
use errors::*;
use github;
use github_rs::client;
use iron;
use std::sync::mpsc;
use std::thread;

#[derive(Clone)]
pub struct Worker {
    tx: mpsc::Sender<Job>,
}

impl Worker {
    pub fn queue_pull_request(&self, job: PullRequestJob) -> Result<()> {
        self.tx.send(Job::PullRequest(job)).chain_err(
            || "Failed to queue pull request",
        )
    }

    pub fn queue_status(&self, state: State, description: String, commit: Commit) -> Result<()> {
        self.tx
            .send(Job::Status(StatusJob {
                status: Status {
                    state: state,
                    description: description,
                    context: "tailor".to_string(),
                },
                commit,
            }))
            .chain_err(|| "Failed to queue status")
    }
}

impl iron::typemap::Key for Worker {
    type Value = Worker;
}

pub enum Job {
    Status(StatusJob),
    PullRequest(PullRequestJob),
}

pub struct StatusJob {
    pub status: Status,
    pub commit: Commit,
}

pub struct PullRequestJob {
    pub owner: String,
    pub repo: String,
    pub number: usize,
    pub head_sha: String,
}

#[derive(Serialize)]
pub struct Status {
    pub state: State,
    pub description: String,
    pub context: String,
}

#[derive(Serialize)]
pub enum State {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "failure")]
    Failure,
    #[serde(rename = "error")]
    Error,
}

pub struct Commit {
    pub owner: String,
    pub repo: String,
    pub sha: String,
}

#[derive(Deserialize)]
pub struct Empty {}

pub fn spawn(config: Config) -> Result<Worker> {
    let (tx, rx) = mpsc::channel::<Job>();

    let worker = Worker { tx };
    let worker_internal = worker.clone();
    thread::Builder::new()
        .name("Status Worker".to_string())
        .spawn(move || {
            let client = client::Github::new(&config.access_token).expect("github client");
            loop {
                match rx.recv() {
                    Ok(Job::Status(job)) => process_status(&client, job),
                    Ok(Job::PullRequest(job)) => {
                        process_pull_request(&client, &config, &worker_internal, job)
                    }
                    Err(err) => eprintln!("Error receiving job: {}", err),
                }
            }
        })
        .chain_err(|| "Failed to start status worker")?;

    Ok(worker)
}

fn process_status(client: &client::Github, job: StatusJob) {
    if let Err(err) = client
        .post(job.status)
        .repos()
        .owner(&job.commit.owner)
        .repo(&job.commit.repo)
        .statuses()
        .sha(&job.commit.sha)
        .execute::<Empty>()
    {
        eprintln!("Failed to set status: {}", err)
    }
}

fn process_pull_request(
    client: &client::Github,
    config: &Config,
    worker: &Worker,
    job: PullRequestJob,
) {
    let (status, description) = match config.repos.iter().find(|curr_repo| {
        &job.owner == &curr_repo.owner && &job.repo == &curr_repo.repo
    }) {
        Some(repo) => {
            match github::validate_pull_request(&job, client, repo) {
                Ok(failures) => {
                    if failures.is_empty() {
                        (State::Success, "All checks passed".to_string())
                    } else {
                        (State::Failure, failures.join("\n"))
                    }
                }
                Err(err) => {
                    eprintln!("Failed to evaluate rules: {:?}", err);
                    (State::Error, format!("Failed to evaluate rules: {}", err))
                }
            }
        }
        None => (
            State::Error,
            format!(
                r#"Could not find repo "{}/{}" in the config"#,
                &job.owner,
                &job.repo
            ),
        ),
    };

    if let Err(err) = worker.queue_status(
        status,
        description,
        Commit {
            owner: job.owner,
            repo: job.repo,
            sha: job.head_sha,
        },
    )
    {
        eprintln!("Failed to queue validation status: {}", err);
    }
}
