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

extern crate regex;

pub struct SummaryScope;

use self::regex::Regex;
use errors::*;
use config::Checks;
use github::CommitData;
use checks::Check;

impl Check for SummaryScope {
    fn name(&self) -> String {
        "summary_scope".to_string()
    }

    fn verify(&self, checks: &Checks, commit_data: &CommitData) -> Result<()> {
        if let Some(summary_scope) = checks.summary_scope {
            if summary_scope {
                let re = Regex::new(r"^.*:\x20.*$").unwrap();
                if !re.is_match(&commit_data.summary) {
                    return Err("Commit summary must contain scope".into());
                }
            }
        }
        Ok(())
    }
}
