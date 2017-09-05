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

use iron::prelude::*;
use iron::status;
use persistent;
use serde_json;
use std::io::Read;
use worker;

#[derive(Serialize, Deserialize)]
struct Hook {
    repository: Repository,
    action: String,
    // for issue comments, we get an issue, for PR event, we get a pull_request. Data is identical
    // though
    issue: Option<Issue>,
    pull_request: Option<Issue>,
}

#[derive(Serialize, Deserialize)]
struct Issue {
    number: usize,
}

#[derive(Serialize, Deserialize)]
struct Repository {
    owner: Owner,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct Owner {
    login: String,
}

pub fn hook_respond(req: &mut Request) -> IronResult<Response> {
    let mut req_string = String::new();
    if let Err(err) = req.body.read_to_string(&mut req_string) {
        eprintln!("Failed to read request body: {}", err);
        return Err(IronError::new(
            err,
            (status::InternalServerError, "Error reading request"),
        ));
    }
    let ds_json: Result<Hook, serde_json::Error> = serde_json::from_str(&req_string);
    match ds_json {
        Ok(ds_json) => {
            let owner = ds_json.repository.owner.login;
            let repo = ds_json.repository.name;
            let number: usize;
            if let Some(issue) = ds_json.issue {
                number = issue.number;
            } else if let Some(pull_request) = ds_json.pull_request {
                number = pull_request.number;
            } else {
                return Ok(Response::with(
                    (status::Ok, "Not an issue comment or PR; ignoring"),
                ));
            }
            let hook_action = ds_json.action;
            if hook_action == "closed" {
                return Ok(Response::with(
                    (status::Ok, "Sent data to processing thread"),
                ));
            }

            let worker = req.get::<persistent::Write<worker::Worker>>().unwrap();
            let tx_clone = match worker.lock() {
                Ok(worker) => worker.get_sender(),
                Err(err) => {
                    return Ok(Response::with((
                        status::InternalServerError,
                        format!("Failed to lock mutex: {}", err),
                    )));
                }
            };

            if let Err(err) = tx_clone.send(worker::Job::Status(worker::StatusJob {
                status: worker::Status {
                    state: worker::State::Pending,
                    description: "The pull request has been received".to_string(),
                    context: "tailor".to_string(),
                },
                commit: worker::Commit {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    sha: "TODO ALEX".to_string(),
                },
            }))
            {
                return Ok(Response::with((
                    status::InternalServerError,
                    format!(
                        "Failed to send struct to processing thread: {}",
                        err
                    ),
                )));
            }

            match tx_clone.send(worker::Job::PullRequest(worker::PullRequestJob {
                owner,
                repo,
                number,
            })) {
                Ok(()) => {
                        Ok(Response::with((status::Ok, "Sent data to processing thread")))
                    }
                Err(err) => {
                    Ok(Response::with((
                        status::InternalServerError,
                        format!(
                            "Failed to send struct to processing thread: {}",
                            err
                        ),
                    )))
                }
            }
        }
        Err(err) => {
            eprintln!("Failed to read json: {}", err);
            Ok(Response::with((
                status::InternalServerError,
                format!("Failed to read input json: {}", err),
            )))
        }
    }
}
