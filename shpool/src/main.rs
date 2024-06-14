// Copyright 2023 Google LLC
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
/// Shpool is a session persistence tool that works simillarly to tmux, but
/// aims to provide a simpler user experience. See [the
/// README](https://github.com/shell-pool/shpool) for more
/// info.
use clap::Parser;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> anyhow::Result<()> {
    // motd::handle_reexec();

    let args = libshpool::Args::parse();

    if args.version() {
        println!("shpool {}", VERSION);
        return Ok(());
    }

    libshpool::run(args, None)
}
