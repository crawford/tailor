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
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate github_rs;
extern crate handlebars_iron;
extern crate iron;
#[macro_use]
extern crate log;
#[macro_use]
extern crate nom;
extern crate params;
extern crate persistent;
extern crate regex;
extern crate router;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate serde_yaml;
extern crate snap;
#[macro_use]
extern crate structopt;
#[macro_use]
extern crate value_derive;

mod config;
mod errors;
mod expr;
mod github;
mod routes;
mod worker;

use errors::*;
use handlebars_iron::{DirectorySource, HandlebarsEngine};
use iron::prelude::*;
use log::LevelFilter;
use router::Router;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Options {
    #[structopt(long = "address", default_value = "0.0.0.0")]
    /// Address on which the server will listen
    pub address: IpAddr,

    #[structopt(long = "port", default_value = "8080")]
    /// Port to which the server will bind
    pub port: u16,

    #[structopt(long = "server-address", default_value = "localhost:8080")]
    /// The socket address used to reach the server
    pub server: SocketAddr,

    #[structopt(long = "templates", default_value = "assets/templates", parse(from_os_str))]
    /// The path to the templates, relative to the working directory
    pub templates: PathBuf,

    #[structopt(long = "token")]
    /// The GitHub access token to use for requests
    pub token: String,

    #[structopt(short = "v", parse(from_occurrences))]
    /// Verbosity level
    pub verbosity: u64,
}

quick_main!(run);

fn run() -> Result<()> {
    let opts = Options::from_args();

    env_logger::Builder::new()
        .filter(
            Some(module_path!()),
            match opts.verbosity {
                0 => LevelFilter::Warn,
                1 => LevelFilter::Info,
                2 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            },
        )
        .init();

    debug!("Spawning worker thread");
    let worker = worker::spawn(opts.token, opts.server.to_string())
        .chain_err(|| "Failed to create status worker")?;

    let mut router = Router::new();
    router.post("/hook", routes::handle_event, "github_webhook");
    router.get("/status", routes::handle_status, "status");

    let mut engine = HandlebarsEngine::new();
    engine.add(Box::new(DirectorySource::new(
        opts.templates,
        Path::new(".hbs").to_path_buf(),
    )));

    engine
        .reload()
        .chain_err(|| "Failed to start templating engine")?;

    let mut chain = Chain::new(router);
    chain.link(persistent::Write::<worker::Worker>::both(worker));
    chain.link_after(engine);

    debug!("Starting web server");
    Iron::new(chain)
        .http((opts.address, opts.port))
        .chain_err(|| "Could not start server")
        .map(|_| ())
}
