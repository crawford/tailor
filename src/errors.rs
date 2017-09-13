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
use github_rs;
use regex;
use serde_json;
use serde_yaml;

error_chain!{
    foreign_links {
        Base64(base64::DecodeError);
        GithubError(github_rs::errors::Error);
        JsonError(serde_json::error::Error);
        RegexError(regex::Error);
        YamlError(serde_yaml::Error);
    }
}
