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

use github_rs::client::Github;
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use CommitStatusEnum;
use CommitStatusEnum::*;
use CommitStatus;
use InitCheckStruct;
use serde_json;
use serde_json::Value;
use github_handler::Commit;
use errors::*;

pub fn set_status(commit_status: CommitStatus) -> Result<()> {
    let mut status_map = HashMap::with_hasher(RandomState::new());
    match commit_status.status {
        Success => {
            status_map.insert(String::from("state"), String::from("success"));
        }
        Pending => {
            status_map.insert(String::from("state"), String::from("pending"));
        }
        Failure => {
            status_map.insert(String::from("state"), String::from("failure"));
        }
        // `Error` gets overridden by `use errors::*`;
        CommitStatusEnum::Error => {
            status_map.insert(String::from("state"), String::from("error"));
        }
    }
    status_map.insert(String::from("description"), commit_status.description);
    status_map.insert(String::from("context"), String::from("tailor"));
    let client = Github::new(commit_status.access_token.as_ref()).chain_err(
        || "Failed to create github client",
    )?;
    client
        .post(status_map)
        .repos()
        .owner(commit_status.owner.as_str())
        .repo(commit_status.repo.as_str())
        .statuses()
        .sha(commit_status.sha.as_str())
        .execute()?;
    Ok(())
}

pub fn set_pending(check_struct: &InitCheckStruct) -> Result<()> {
    let client = Github::new(check_struct.access_token.as_ref()).chain_err(
        || "Failed to create github client",
    )?;

    let commits = client
        .get()
        .repos()
        .owner(check_struct.owner.as_str())
        .repo(check_struct.repo.as_str())
        .pulls()
        .number(check_struct.number.to_string().as_str())
        .commits()
        .execute();

    let ds_json: Value;
    match commits {
        Ok((_, _, Some(json))) => {
            ds_json = json;
        }
        Ok((_, _, None)) => {
            return Err("Could not get PR commit data!".into());
        }
        Err(e) => {
            return Err(e.into());
        }
    }

    let json_arr: Vec<Commit> = serde_json::from_value(ds_json).chain_err(|| {
        "Failed to deserialize PR JSON (it is possible that this was trigger by an issue comment
        not related to a PR)"
    })?;

    for commit in &json_arr {
        let status_struct = CommitStatus {
            owner: check_struct.owner.clone(),
            repo: check_struct.repo.clone(),
            sha: commit.sha.clone(),
            status: Pending,
            description: "Doing commit check".to_string(),
            access_token: check_struct.access_token.clone(),
        };
        if let Err(e) = set_status(status_struct) {
            eprintln!("{}", e);
        }
    }
    Ok(())
}
