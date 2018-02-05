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
use config;
use errors::*;
use expr;
use expr::ast::Value;
use github::TryExecute;
use github_rs::client::Github;
use github::types;
use serde_yaml;
use worker;

#[derive(Value)]
struct PullRequest {
    user: types::User,
    title: String,
    body: Option<String>,
    commits: Vec<Commit>,
    comments: Vec<types::Comment>,
    base: types::CommitReference,
    head: types::CommitReference,
}

#[derive(Value)]
struct Commit {
    sha: String,
    author: types::Author,
    committer: types::Author,
    title: String,
    description: String,
}

pub fn pull_request(job: &worker::PullRequestJob, client: &Github) -> Result<Vec<String>> {
    let pr = fetch_pull_request(client, &job.owner, &job.repo, job.number)?;
    let repo = fetch_repo_config(client, &job.owner, &job.repo, &pr)?;
    let exemptions = find_exemptions(client, &job.owner, &job.repo, &pr)?;

    let mut failures = Vec::new();
    let input = pr.into();
    for rule in repo.rules
        .iter()
        .filter(|rule| !exemptions.contains(&rule.name))
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

fn fetch_repo_config(
    client: &Github,
    owner: &str,
    repo: &str,
    pr: &PullRequest,
) -> Result<config::Config> {
    trace!("Fetching repo config for {}/{}", owner, repo);
    let config: types::Content = client
        .get()
        .repos()
        .owner(owner)
        .repo(repo)
        .contents()
        .path(".github/tailor.yaml")
        .reference(&pr.head.sha)
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
            Ok(config::Config { rules: Vec::new() })
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
                let collaborator: types::Collaborator = client
                    .get()
                    .repos()
                    .owner(owner)
                    .repo(repo)
                    .collaborators()
                    .username(&comment.user.login)
                    .permission()
                    .try_execute()
                    .chain_err(|| "Failed to fetch collaborator data")?;
                if collaborator.permission == types::Permission::Admin {
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
    let pr: types::PullRequest = client
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
        let raw_commits: Vec<types::Commit> = client
            .get()
            .repos()
            .owner(owner)
            .repo(repo)
            .pulls()
            .number(&number.to_string())
            .commits()
            .try_execute()
            .chain_err(|| "Failed to fetch pull request commits")?;

        let mut commits = Vec::new();
        for c in raw_commits {
            let (title, description) = {
                let mut lines = c.commit.message.lines();
                let title = lines.next().expect("at least one line").to_string();
                match lines.next() {
                    Some("") | None => {}
                    _ => return Err(
                        "Malformed commit message (no empty line between title and description)"
                            .into(),
                    ),
                }
                let description = lines.collect::<Vec<_>>().as_slice().join("\n");
                (title, description)
            };

            commits.push(Commit {
                sha: c.sha,
                author: types::Author {
                    name: c.commit.author.name,
                    email: c.commit.author.email,
                    date: c.commit.author.date,
                    github_login: Some(c.author.login),
                },
                committer: types::Author {
                    name: c.commit.committer.name,
                    email: c.commit.committer.email,
                    date: c.commit.committer.date,
                    github_login: Some(c.committer.login),
                },
                title,
                description,
            })
        }
        commits
    };

    trace!("Fetching pull request comments");
    let comments: Vec<types::Comment> = client
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
        base: pr.base,
        head: pr.head,
        commits,
        comments,
    })
}
