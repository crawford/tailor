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

use std::io::Read;
use iron::request::Request;
use iron::prelude::*;
use iron::typemap::Key;
use iron::status;
use persistent;
use serde_json;
use config;
use std::sync::mpsc;
use InitCheckStruct;

use iron::response::Response;

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

#[derive(Copy, Clone)]
pub struct Config;
impl Key for Config {
    type Value = config::Config;
}

#[derive(Copy, Clone)]
pub struct AccessToken;
impl Key for AccessToken {
    type Value = String;
}

#[derive(Copy, Clone)]
pub struct Tx;
impl Key for Tx {
    type Value = mpsc::Sender<InitCheckStruct>;
}

pub fn hook_respond(req: &mut Request) -> IronResult<Response> {
    let mut req_string = String::new();
    if let Err(e) = req.body.read_to_string(&mut req_string) {
        eprintln!("Failed to read request body => {}", e);
        return Err(IronError::new(
            e,
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


            let config_req = req.get::<persistent::Read<Config>>().unwrap();
            let access_token_req = req.get::<persistent::Read<AccessToken>>().unwrap();
            let tx = req.get::<persistent::Write<Tx>>().unwrap();

            let job_struct = InitCheckStruct {
                owner: owner,
                repo: repo,
                number: number,
                hook_action: hook_action,
                config: config_req,
                access_token: access_token_req,
            };

            let tx_clone = {
                let tx_lock = match tx.lock() {
                    Ok(tx) => tx,
                    Err(e) => {
                        return Ok(Response::with((
                            status::InternalServerError,
                            format!("Failed to lock mutex => {}", e),
                        )));
                    }
                };
                tx_lock.clone()
            };

            match tx_clone.send(job_struct) {
                Ok(()) => {
                        Ok(Response::with((status::Ok, "Sent data to processing thread")))
                    }
                Err(e) => {
                    Ok(Response::with((
                        status::InternalServerError,
                        format!(
                            "Failed to send struct to processing thread => {}",
                            e
                        ),
                    )))
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to read json => {}", e);
            Ok(Response::with((
                status::InternalServerError,
                format!("Failed to read input json => {}", e),
            )))
        }
    }
}
