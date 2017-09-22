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

use errors::*;
use github::{self, TryExecute};
use github_rs::client;
use iron;
use std::fmt;
use std::sync::mpsc;
use std::thread;

impl<'a> TryExecute for ::github_rs::repos::post::Sha<'a> {}

#[derive(Clone)]
pub struct Worker {
    tx: mpsc::Sender<Job>,
}

impl Worker {
    pub fn queue_pull_request(&self, job: PullRequestJob) -> Result<()> {
        debug!("Queuing pull request {:?}", job);
        self.tx.send(Job::PullRequest(job)).chain_err(
            || "Failed to queue pull request",
        )
    }

    pub fn queue_status(&self, state: State, description: String, commit: Commit) -> Result<()> {
        debug!("Queuing status {:?} for {:?}", state, commit);
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

impl fmt::Debug for PullRequestJob {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Pull Request {}/{}: {} ({})",
            self.owner,
            self.repo,
            self.number,
            self.head_sha
        )
    }
}

#[derive(Serialize)]
pub struct Status {
    pub state: State,
    pub description: String,
    pub context: String,
}

#[derive(Debug, Serialize)]
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

impl fmt::Debug for Commit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Commit {}/{}: {}", self.owner, self.repo, self.sha)
    }
}

#[derive(Deserialize)]
pub struct Empty {}

pub fn spawn(access_token: String) -> Result<Worker> {
    let (tx, rx) = mpsc::channel::<Job>();

    let worker = Worker { tx };
    let worker_internal = worker.clone();
    thread::Builder::new()
        .name("Status Worker".to_string())
        .spawn(move || {
            let client = client::Github::new(&access_token).expect("github client");
            loop {
                match rx.recv() {
                    Ok(Job::Status(job)) => process_status(&client, job),
                    Ok(Job::PullRequest(job)) => {
                        process_pull_request(&client, &worker_internal, job)
                    }
                    Err(err) => error!("Error receiving job: {}", err),
                }
            }
        })
        .chain_err(|| "Failed to start status worker")?;

    Ok(worker)
}

fn process_status(client: &client::Github, job: StatusJob) {
    debug!(
        "Processing status {:?} for {:?}",
        job.status.state,
        job.commit
    );

    if let Err(err) = client
        .post(job.status)
        .repos()
        .owner(&job.commit.owner)
        .repo(&job.commit.repo)
        .statuses()
        .sha(&job.commit.sha)
        .try_execute::<Empty>()
    {
        error!("Failed to set status: {}", err)
    }
}

fn process_pull_request(client: &client::Github, worker: &Worker, job: PullRequestJob) {
    debug!("Processing pull request {:?}", job);

    let (status, description) = {
        match github::validate_pull_request(&job, client) {
            Ok(failures) => {
                debug!("Validation results: {:?}", failures);
                if failures.is_empty() {
                    (State::Success, "All checks passed".to_string())
                } else {
                    (State::Failure, failures.join("\n"))
                }
            }
            Err(err) => {
                warn!("Failed to evaluate rules: {}", err);
                (State::Error, format!("Failed to evaluate rules: {}", err))
            }
        }
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
        error!("Failed to queue validation status: {}", err);
    }
}
