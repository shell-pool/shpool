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

//! The common module is a grab bag of shared utility functions.

use std::env;

use anyhow::anyhow;

pub fn resolve_sessions(sessions: &mut Vec<String>, action: &str) -> anyhow::Result<()> {
    if sessions.len() == 0 {
        if let Ok(current_session) = env::var("SHPOOL_SESSION_NAME") {
            sessions.push(current_session);
        }
    }

    if sessions.len() == 0 {
        eprintln!("no session to {}", action);
        return Err(anyhow!("no session to {}", action));
    }

    Ok(())
}
