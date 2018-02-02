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

pub mod types;
pub mod validate;

use errors::*;
use github::types::ErrorResponse;
use github_rs::StatusCode;
use github_rs::client::Executor;
use serde::de::DeserializeOwned;
use serde_json;

pub trait TryExecute: Executor {
    fn try_execute<T: DeserializeOwned>(self) -> Result<T>
    where
        Self: Sized,
    {
        match self.execute::<serde_json::Value>() {
            Ok((_, StatusCode::Ok, Some(response)))
            | Ok((_, StatusCode::Created, Some(response))) => {
                serde_json::from_value(response).chain_err(|| "Failed to parse response")
            }
            Ok((_, _, Some(response))) => serde_json::from_value::<ErrorResponse>(response)
                .chain_err(|| "Failed to parse error response")
                .and_then(|error| {
                    debug!("Failed to complete request: {:?}", error);
                    Err(error.message.into())
                }),
            Ok((_, _, None)) => Err("Received error response from github with no message".into()),
            Err(err) => Err(err).chain_err(|| "Failed to execute request"),
        }.or_else(|err| {
            error!("Failed to complete request: {}", err);
            Err(err)
        })
    }
}

impl<'a> TryExecute for ::github_rs::repos::get::ContentsReference<'a> {}
impl<'a> TryExecute for ::github_rs::repos::get::PullsNumber<'a> {}
impl<'a> TryExecute for ::github_rs::repos::get::CollaboratorsUsernamePermission<'a> {}
impl<'a> TryExecute for ::github_rs::repos::get::IssuesNumberComments<'a> {}
impl<'a> TryExecute for ::github_rs::repos::get::PullsNumberCommits<'a> {}
