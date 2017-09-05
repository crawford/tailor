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

pub struct Worker {
    tx: mpsc::Sender<Job>,
    handle: thread::JoinHandle<()>,
}

impl Worker {
    pub fn queue(&self, job: Job) -> Result<()> {
        self.tx.send(job).chain_err(|| "Failed to queue job")
    }

    pub fn get_sender(&self) -> mpsc::Sender<Job> {
        self.tx.clone()
    }
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
}

#[derive(Serialize)]
pub struct Status {
    pub state: State,
    pub description: String,
    pub context: String,
}

#[derive(Serialize)]
pub enum State {
    Success,
    Pending,
    Failure,
    Error,
}

pub struct Commit {
    pub owner: String,
    pub repo: String,
    pub sha: String,
}

pub fn spawn(config: Config) -> Result<Worker> {
    let (tx, rx) = mpsc::channel::<Job>();

    let tx_internal = tx.clone();
    let handle = thread::Builder::new()
        .name("Status Worker".to_string())
        .spawn(move || {
            let client = client::Github::new(&config.access_token).expect("github client");
            loop {
                match rx.recv() {
                    Ok(Job::Status(job)) => {
                        if let Err(err) = client
                            .post(job.status)
                            .repos()
                            .owner(&job.commit.owner)
                            .repo(&job.commit.repo)
                            .statuses()
                            .sha(&job.commit.sha)
                            .execute()
                        {
                            eprintln!("Failed to set status: {}", err);
                        }
                    }
                    Ok(Job::PullRequest(job)) => {
                        if let Err(err) = github::run_checks(&job, &tx_internal, &config) {
                            eprintln!("{}", err);
                        }
                    }
                    Err(err) => eprintln!("Error receiving job: {}", err),
                }
            }
        })
        .chain_err(|| "Failed to start status worker")?;

    Ok(Worker { tx, handle })
}

impl iron::typemap::Key for Worker {
    type Value = Worker;
}
