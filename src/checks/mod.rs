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

pub mod no_capitalize_summary;
pub mod max_summary_length;
pub mod summary_scope;
pub mod max_body_line_length;
pub mod requires_body;
pub mod no_wip;
pub mod no_fixup;
pub mod no_squash;

use errors::*;
use config::Checks;
use github::CommitData;

pub trait Check {
    fn name(&self) -> String;

    fn verify(&self, checks: &Checks, commit_data: &CommitData) -> Result<()>;
}
