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

pub struct NoCapitalizeSummary;

use self::regex::Regex;
use errors::*;
use config::Checks;
use github::CommitData;
use checks::Check;

impl Check for NoCapitalizeSummary {
    fn name(&self) -> String {
        "no_capitalize_summary".to_string()
    }

    fn verify(&self, checks: &Checks, commit_data: &CommitData) -> Result<()> {
        if let Some(no_capitalize_summary) = checks.no_capitalize_summary {
            if no_capitalize_summary {
                let re = Regex::new(r"^[^A-Z]*$").unwrap();
                if !re.is_match(&commit_data.summary) {
                    return Err("Commit summary must be lowercase".into());
                }
            }
        }
        Ok(())
    }
}
