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

pub struct MaxSummaryLength;

use errors::*;
use config::Checks;
use github::CommitData;
use checks::Check;

impl Check for MaxSummaryLength {
    fn name(&self) -> String {
        "max_summary_length".to_string()
    }

    fn verify(&self, checks: &Checks, commit_data: &CommitData) -> Result<()> {
        if let Some(length) = checks.max_summary_length {
            if commit_data.summary.len() > length {
                return Err(
                    format!("Commit summary is longer than {} characters", length).into(),
                );
            }
        }
        Ok(())
    }
}
