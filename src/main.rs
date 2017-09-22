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

extern crate base64;
extern crate chrono;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate value_derive;
#[macro_use]
extern crate error_chain;
extern crate github_rs;
extern crate iron;
#[macro_use]
extern crate log;
extern crate loggerv;
#[macro_use]
extern crate nom;
extern crate persistent;
extern crate regex;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde_yaml;

mod config;
mod errors;
mod expr;
mod github;
mod routes;
mod worker;

use clap::{Arg, App};
use errors::*;
use iron::prelude::*;
use router::Router;
use std::str::FromStr;
use std::process;

quick_main!(run);

fn run() -> Result<()> {
    let args = App::new(crate_name!())
        .version(crate_version!())
        .about(crate_description!())
        .arg(
            Arg::with_name("ADDRESS")
                .short("a")
                .long("address")
                .takes_value(true)
                .help("The address on which to listen")
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("PORT")
                .short("p")
                .long("port")
                .takes_value(true)
                .help("The port on which to bind")
                .default_value("8080"),
        )
        .arg(
            Arg::with_name("TOKEN")
                .short("t")
                .long("token")
                .takes_value(true)
                .required(true)
                .help("The GitHub access token to use for requests"),
        )
        .arg(
            Arg::with_name("VERBOSITY")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("The level of verbosity"),
        )
        .get_matches();

    loggerv::init_with_verbosity(args.occurrences_of("VERBOSITY")).unwrap();

    let address = args.value_of("ADDRESS").expect("address flag");
    let port = match u16::from_str(args.value_of("PORT").expect("port flag")) {
        Ok(port) => port,
        Err(err) => {
            eprintln!("Failed to parse port: {}", err);
            process::exit(2)
        }
    };

    debug!("Spawning worker thread");
    let worker = worker::spawn(args.value_of("TOKEN").expect("token").to_string())
        .chain_err(|| "Failed to create status worker")?;

    let mut router = Router::new();
    router.post("/hook", routes::hook_respond, "github_webhook");

    let mut chain = Chain::new(router);
    chain.link(persistent::Write::<worker::Worker>::both(worker));

    debug!("Starting web server");
    Iron::new(chain)
        .http((address, port))
        .chain_err(|| "Could not start server")
        .map(|_| ())
}
