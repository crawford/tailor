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

use base64;
use chrono::prelude::*;
use config;
use errors::*;
use expr;
use expr::ast::Value;
use github_rs::StatusCode;
use github_rs::client::{Executor, Github};
use serde::de::DeserializeOwned;
use serde_json;
use serde_yaml;
use worker;

#[derive(Clone, Value)]
struct PullRequest {
    user: User,
    title: String,
    body: String,
    commits: Vec<Commit>,
    comments: Vec<Comment>,

    #[value(hidden)]
    head_sha: String,
}

#[derive(Clone, Deserialize, Value)]
struct Author {
    name: String,
    email: String,
    date: DateTime<Utc>,
    github_login: Option<String>,
}

#[derive(Clone, Deserialize, Value)]
struct Comment {
    user: User,
    body: String,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Value)]
struct Commit {
    sha: String,
    author: Author,
    committer: Author,
    message: String,
}

#[derive(Clone, Deserialize, Value)]
struct User {
    login: String,
}

#[derive(Deserialize)]
struct RawPullRequest {
    user: User,
    title: String,
    body: String,
    head: RawNakedCommit,
}

#[derive(Clone, Deserialize)]
struct RawCommit {
    sha: String,
    commit: RawCommitBody,
    author: User,
    committer: User,
}

#[derive(Clone, Deserialize)]
struct RawNakedCommit {
    sha: String,
}

#[derive(Clone, Deserialize)]
struct RawCommitBody {
    author: Author,
    committer: Author,
    message: String,
}

#[derive(Deserialize)]
struct RawContent {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Collaborator {
    permission: Permission,
}

#[derive(Deserialize, PartialEq)]
enum Permission {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "write")]
    Write,
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "none")]
    None,
}

pub fn validate_pull_request(job: &worker::PullRequestJob, client: &Github) -> Result<Vec<String>> {
    let pr = fetch_pull_request(client, &job.owner, &job.repo, job.number)?;
    let repo = fetch_repo_config(client, &job.owner, &job.repo, &pr)?;
    let exemptions = find_exemptions(client, &job.owner, &job.repo, &pr)?;

    let mut failures = Vec::new();
    let input = pr.clone().into();
    for rule in repo.rules.iter().filter(
        |rule| !exemptions.contains(&rule.name),
    )
    {
        let result = expr::eval(&rule.expression, &input).chain_err(|| {
            format!(
                r#"Failed to run "{}" from "{}/{}""#,
                rule.name,
                job.owner,
                job.repo
            )
        })?;

        if !result {
            failures.push(format!("Failed {} ({})", rule.name, rule.description))
        }
    }
    Ok(failures)
}

pub trait TryExecute: Executor {
    fn try_execute<T: DeserializeOwned>(self) -> Result<T>
    where
        Self: Sized,
    {
        #[derive(Debug, Deserialize)]
        struct GithubErrorResponse {
            message: String,
            errors: Option<Vec<GithubError>>,
        }

        #[derive(Debug, Deserialize)]
        struct GithubError {
            resource: String,
            field: String,
            code: String,
            message: Option<String>,
        }

        match self.execute::<serde_json::Value>() {
            Ok((_, StatusCode::Ok, Some(response))) |
            Ok((_, StatusCode::Created, Some(response))) => {
                serde_json::from_value(response).chain_err(|| "Failed to parse response")
            }
            Ok((_, _, Some(response))) => {
                serde_json::from_value::<GithubErrorResponse>(response)
                    .chain_err(|| "Failed to parse error response")
                    .and_then(|error| {
                        debug!("Failed to complete request: {:?}", error);
                        Err(error.message.into())
                    })
            }
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

fn fetch_repo_config(
    client: &Github,
    owner: &str,
    repo: &str,
    pr: &PullRequest,
) -> Result<config::Config> {
    trace!("Fetching repo config for {}/{}", owner, repo);
    let config: RawContent = client
        .get()
        .repos()
        .owner(owner)
        .repo(repo)
        .contents()
        .path(".github/tailor.yaml")
        .reference(&pr.head_sha)
        .try_execute()
        .chain_err(|| {
            format!("Failed to fetch repo configuration for {}/{}", owner, repo)
        })?;
    match config.content {
        Some(content) => Ok(serde_yaml::from_slice(
            &base64::decode_config(&content, base64::MIME)?,
        )?),
        None => {
            warn!("Repository {}/{} has no tailor configuration", owner, repo);
            return Ok(config::Config { rules: Vec::new() });
        }
    }
}

fn find_exemptions(
    client: &Github,
    owner: &str,
    repo: &str,
    pr: &PullRequest,
) -> Result<Vec<String>> {
    let mut exemptions = Vec::new();
    for comment in &pr.comments {
        // TODO ALEX
        if (&(comment.body)).starts_with("tailor disable") {
            let mut split = comment.body.as_str().split("tailor disable");
            split.next();
            if let Some(disabled_check) = split.next() {
                trace!(
                    "Fetching repo collaborator status for {}",
                    comment.user.login
                );
                let collaborator: Collaborator =
                    client
                        .get()
                        .repos()
                        .owner(owner)
                        .repo(repo)
                        .collaborators()
                        .username(&comment.user.login)
                        .permission()
                        .try_execute()
                        .chain_err(|| "Failed to fetch collaborator data")?;
                if collaborator.permission == Permission::Admin {
                    exemptions.push(disabled_check.trim().to_string());
                }
            }
        }
    }

    Ok(exemptions)
}

fn fetch_pull_request(
    client: &Github,
    owner: &str,
    repo: &str,
    number: usize,
) -> Result<PullRequest> {
    trace!("Fetching pull request {}/{}: {}", owner, repo, number);
    let pr: RawPullRequest = client
        .get()
        .repos()
        .owner(owner)
        .repo(repo)
        .pulls()
        .number(&number.to_string())
        .try_execute()
        .chain_err(|| "Failed to fetch pull request")?;

    let commits = {
        trace!("Fetching pull request commits");
        let raw_commits: Vec<RawCommit> =
            client
                .get()
                .repos()
                .owner(owner)
                .repo(repo)
                .pulls()
                .number(&number.to_string())
                .commits()
                .try_execute()
                .chain_err(|| "Failed to fetch pull request commits")?;

        raw_commits
            .into_iter()
            .map(|c: RawCommit| {
                Commit {
                    sha: c.sha,
                    author: Author {
                        name: c.commit.author.name,
                        email: c.commit.author.email,
                        date: c.commit.author.date,
                        github_login: Some(c.author.login),
                    },
                    committer: Author {
                        name: c.commit.committer.name,
                        email: c.commit.committer.email,
                        date: c.commit.committer.date,
                        github_login: Some(c.committer.login),
                    },
                    message: c.commit.message,
                }
            })
            .collect()
    };

    trace!("Fetching pull request comments");
    let comments: Vec<Comment> = client
        .get()
        .repos()
        .owner(owner)
        .repo(repo)
        .issues()
        .number(&number.to_string())
        .comments()
        .try_execute()
        .chain_err(|| "Failed to fetch pull request comments")?;

    Ok(PullRequest {
        user: pr.user,
        title: pr.title,
        body: pr.body,
        head_sha: pr.head.sha,
        commits,
        comments,
    })
}
