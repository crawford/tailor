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

use github::types::Event;
use iron::prelude::*;
use iron::status;
use persistent;
use serde_json;
use std::io::Read;
use worker;

pub fn handle_event(req: &mut Request) -> IronResult<Response> {
    let event: Event = {
        let mut body = String::new();
        req.body.read_to_string(&mut body).map_err(|err| {
            error!("Failed to read GitHub request: {}", err);
            IronError::new(err, (status::InternalServerError, "Failed to read request"))
        })?;
        serde_json::from_str(&body)
    }.map_err(|err| {
        error!("Failed to parse GitHub request: {}", err);
        IronError::new(err, (
            status::InternalServerError,
            "Failed to parse response body",
        ))
    })?;

    info!("Received GitHub event: {:?}", event);

    if event.hook.is_some() {
        debug!("Received GitHub event for hook registration");
        return Ok(Response::with(status::Ok));
    };

    let pull_request = match event.pull_request {
        Some(pull_request) => pull_request,
        None => {
            info!("Received GitHub event for something other than a pull request; ignoring.");
            return Ok(Response::with((status::Ok, "Not a pull request")));
        }
    };

    if event.action == Some("closed".into()) {
        debug!("Received GitHub request for closed pull request; ignoring.");
        return Ok(Response::with((status::Ok, "Ignoring closed pull request")));
    }

    let w = req.get::<persistent::Write<worker::Worker>>().unwrap();
    let worker = match w.lock() {
        Ok(worker) => worker,
        Err(err) => {
            error!("Failed to aquire worker.");
            return Ok(Response::with((
                status::InternalServerError,
                format!("Failed to aquire worker: {}", err),
            )));
        }
    };

    if let Err(err) = worker.queue_status(
        worker::State::Pending,
        "The pull request has been received".to_string(),
        worker::Commit {
            owner: event.repository.owner.login.clone(),
            repo: event.repository.name.clone(),
            sha: pull_request.head.sha.clone(),
        },
    )
    {
        error!("Failed to queue status: {}", err);
        return Ok(Response::with((
            status::InternalServerError,
            format!(
                "Failed to send struct to processing thread: {}",
                err
            ),
        )));
    }

    if let Err(err) = worker.queue_pull_request(worker::PullRequestJob {
        owner: event.repository.owner.login,
        repo: event.repository.name,
        number: pull_request.number,
        head_sha: pull_request.head.sha,
    })
    {
        error!("Failed to queue pull request: {}", err);
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
