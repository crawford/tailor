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

use chrono::prelude::*;
use expr::ast::Value;

#[derive(Clone, Deserialize, Value)]
pub struct Author {
    pub name: String,
    pub email: String,
    pub date: DateTime<Utc>,
    pub github_login: Option<String>,
}

#[derive(Deserialize)]
pub struct Collaborator {
    pub permission: Permission,
}

#[derive(Clone, Deserialize, Value)]
pub struct Comment {
    pub user: User,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct Commit {
    pub sha: String,
    pub commit: CommitBody,
    pub author: User,
    pub committer: User,
}

#[derive(Deserialize)]
pub struct CommitBody {
    pub author: Author,
    pub committer: Author,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CommitReference {
    pub sha: String,
    pub user: User,
}

#[derive(Deserialize)]
pub struct Content {
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Empty {}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
    pub errors: Option<Vec<Error>>,
}

#[derive(Debug, Deserialize)]
pub struct Error {
    pub resource: String,
    pub field: String,
    pub code: String,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub repository: Repository,
    pub action: Option<String>,
    pub hook: Option<Empty>,
    pub pull_request: Option<PullRequest>,
}

#[derive(Deserialize, PartialEq)]
pub enum Permission {
    #[serde(rename = "admin")] Admin,
    #[serde(rename = "write")] Write,
    #[serde(rename = "read")] Read,
    #[serde(rename = "none")] None,
}

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub user: User,
    pub number: usize,
    pub title: String,
    pub body: Option<String>,
    pub head: CommitReference,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub owner: User,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Value)]
pub struct User {
    pub login: String,
}
