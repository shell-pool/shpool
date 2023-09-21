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
