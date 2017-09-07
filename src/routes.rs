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
use iron::prelude::*;
use iron::status;
use persistent;
use serde_json;
use urlencoded::UrlEncodedBody;
use worker;

#[derive(Deserialize)]
struct Hook {
    repository: Repository,
    action: String,
    pull_request: Option<PullRequest>,
}

#[derive(Deserialize)]
struct PullRequest {
    number: usize,
    head: Commit,
}

#[derive(Deserialize)]
struct Commit {
    sha: String,
}

#[derive(Deserialize)]
struct Repository {
    owner: Owner,
    name: String,
}

#[derive(Deserialize)]
struct Owner {
    login: String,
}



fn read_body(req: &mut Request) -> Result<String> {
    req.get_ref::<UrlEncodedBody>()
        .chain_err(|| "Failed to URL decode")?
        .clone()
        .remove("payload")
        .ok_or("Failed to find payload")?
        .pop()
        .ok_or("Empty payload".into())
}

pub fn hook_respond(req: &mut Request) -> IronResult<Response> {
    let payload: Hook = serde_json::from_str(&read_body(req).map_err(|err| {
        IronError::new(
            err,
            (status::InternalServerError, "Failed to read response"),
        )
    })?).map_err(|err| {
        eprintln!("Failed to parse response body: {:?}", err);
        IronError::new(err, (
            status::InternalServerError,
            "Failed to parse response body",
        ))
    })?;

    let pull_request = match payload.pull_request {
        Some(pull_request) => pull_request,
        None => return Ok(Response::with((status::Ok, "Not a pull request"))),
    };

    if payload.action == "closed" {
        return Ok(Response::with((status::Ok, "Ignoring closed pull request")));
    }

    let w = req.get::<persistent::Write<worker::Worker>>().unwrap();
    let worker = match w.lock() {
        Ok(worker) => worker,
        Err(err) => {
            return Ok(Response::with((
                status::InternalServerError,
                format!("Failed to lock mutex: {}", err),
            )));
        }
    };

    if let Err(err) = worker.queue_status(
        worker::State::Pending,
        "The pull request has been received".to_string(),
        worker::Commit {
            owner: payload.repository.owner.login.clone(),
            repo: payload.repository.name.clone(),
            sha: pull_request.head.sha.clone(),
        },
    )
    {
        return Ok(Response::with((
            status::InternalServerError,
            format!(
                "Failed to send struct to processing thread: {}",
                err
            ),
        )));
    }

    if let Err(err) = worker.queue_pull_request(worker::PullRequestJob {
        owner: payload.repository.owner.login,
        repo: payload.repository.name,
        number: pull_request.number,
        head_sha: pull_request.head.sha,
    })
    {
        return Ok(Response::with((
            status::InternalServerError,
            format!(
                "Failed to send struct to processing thread: {}",
                err
            ),
        )));
    }

    Ok(Response::with(
        (status::Ok, "Sent data to processing thread"),
    ))
}
