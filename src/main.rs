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

mod config;
mod github_handler;
mod checks;
mod routes;

extern crate github_rs;
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;
extern crate iron;
extern crate persistent;
extern crate router;
#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate clap;

// We'll put our errors in an `errors` module, and other modules in
// this crate will `use errors::*;` to get access to everything
// `error_chain!` creates.
mod errors {
    // handle json deserialization errors
    use serde_json;
    use github_rs;

    error_chain!{
        foreign_links {
            JsonError(serde_json::error::Error);
            GithubError(github_rs::errors::Error);
        }
    }
}

use errors::*;
use std::str;
use iron::prelude::*;
use clap::{Arg, App};
use router::Router;
use std::net::IpAddr;
use std::str::FromStr;
use std::thread;
use std::sync::{Arc, mpsc};

pub enum CommitStatusEnum {
    Success,
    Pending,
    Failure,
    Error,
}

pub struct SetStatus {
    pub check_struct: Option<InitCheckStruct>,
    pub commit_status: Option<CommitStatus>,
}

pub struct CommitStatus {
    pub owner: String,
    pub repo: String,
    pub sha: String,
    pub status: CommitStatusEnum,
    pub description: String,
    pub access_token: Arc<String>,
}

pub struct InitCheckStruct {
    pub owner: String,
    pub repo: String,
    pub number: usize,
    pub hook_action: String,
    pub config: Arc<config::Config>,
    pub access_token: Arc<String>,
}

quick_main!(run);

fn run() -> Result<()> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true)
                .help("The address on which to listen")
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .takes_value(true)
                .help("The port on which to listen")
                .default_value("3000"),
        )
        .get_matches();

    // This channel handles interaction between the hook handler and scheduling thread
    let (tx_schedule, rx_schedule) = mpsc::channel::<InitCheckStruct>();
    // This channel handles communication between the scheduling and processing/check threads to the
    // status thread
    let (tx_status, rx_status) = mpsc::channel::<SetStatus>();
    // This handles communication from the scheduling thread to the processing/check thread
    let (tx_check, rx_check) = mpsc::channel::<InitCheckStruct>();
    // Scheduling thread (there are 2 threads which need tx_status, so we'll clone it here)
    let tx_status_clone = tx_status.clone();
    thread::spawn(move || loop {
        // The type must be specified for the `if job.hook_action` line to work
        // Also, override errors::Result;
        let job = rx_schedule.recv();
        match job {
            Ok(job) => {
                // Don't do anything if the PR is just being closed
                if job.hook_action == "closed" {
                    continue;
                }
                // Set statuses to pending (and make copy of job)
                let job_copy = InitCheckStruct {
                    owner: job.owner.clone(),
                    repo: job.repo.clone(),
                    number: job.number,
                    hook_action: job.hook_action.clone(),
                    config: job.config.clone(),
                    access_token: job.access_token.clone(),
                };
                if let Err(e) = tx_status_clone.send(SetStatus {
                    check_struct: Some(job_copy),
                    commit_status: None,
                })
                {
                    eprintln!(
                        "Failed to send status pending data to status thread => {}",
                        e
                    );
                    continue;
                }
                // Run the checks
                if let Err(e) = tx_check.send(job) {
                    eprintln!("Failed to send job data to check thread => {}", e);
                    continue;
                };
            }
            Err(e) => eprintln!("Error getting job on schedule thread => {}", e),
        }
    });
    // Status thread
    thread::spawn(move || loop {
        let job = rx_status.recv();
        match job {
            Ok(job) => {
                if let Some(check_struct) = job.check_struct {
                    if let Err(e) = github_handler::set_status::set_pending(&check_struct) {
                        eprintln!("{}", e);
                    }
                } else if let Some(commit_status) = job.commit_status {
                    if let Err(e) = github_handler::set_status::set_status(commit_status) {
                        eprintln!("{}", e);
                    }
                } else {
                    eprintln!("Invalid SetStatus struct");
                }
            }
            Err(e) => eprintln!("Error getting job on status thread => {}", e),
        }
    });
    // Processing/Check thread
    thread::spawn(move || loop {
        let job = rx_check.recv();
        match job {
            Ok(job) => {
                if let Err(e) = github_handler::run_checks(&job, &tx_status) {
                    eprintln!("{}", e);
                }
            }
            Err(e) => eprintln!("Error getting job on check thread => {}", e),
        }
    });

    let mut router = Router::new();
    router.post("/hook", routes::hook::hook_respond, "github_webhook");

    let config = config::get_config()?;

    let mut chain = Chain::new(router);
    chain.link(persistent::Read::<routes::hook::AccessToken>::both(
        config.access_token.clone(),
    ));
    chain.link(persistent::Read::<routes::hook::Config>::both(config));
    chain.link(persistent::Write::<routes::hook::Tx>::both(tx_schedule));

    let address = matches.value_of("address");
    let port = matches.value_of("port");
    if let (Some(address), Some(port)) = (address, port) {
        let port = u16::from_str_radix(port, 10).chain_err(|| "Invalid port")?;
        let address = IpAddr::from_str(address).chain_err(|| "Invalid address")?;
        Iron::new(chain).http((address, port)).chain_err(
            || "Could not start server",
        )?;
    } else {
        eprintln!("Could not decode command line arguments");
    }
    Ok(())
}
