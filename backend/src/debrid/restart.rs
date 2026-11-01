#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeRemote {
    PollExisting,
    RefreshLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartDecision {
    Preserve,
    ResumeExistingRemote(ResumeRemote),
    ResumeArrConfirmation,
    ResetQueued,
    Terminal,
}

#[derive(Debug, Clone, Copy)]
pub struct RestartInput<'a> {
    pub execution_strategy: Option<&'a str>,
    pub external_id: Option<&'a str>,
    pub state: &'a str,
    pub remote_state: Option<&'a str>,
}

pub fn restart_decision(input: RestartInput<'_>) -> Result<RestartDecision, &'static str> {
    if matches!(input.state, "QUEUED" | "PAUSED") {
        return Ok(RestartDecision::Preserve);
    }
    if matches!(input.state, "COMPLETED" | "FAILED") {
        return Ok(RestartDecision::Terminal);
    }
    if input.state == "IMPORTING" {
        return Ok(RestartDecision::ResumeArrConfirmation);
    }
    if !matches!(input.state, "STARTING" | "DOWNLOADING") {
        return Err("unknown restart state");
    }
    match input.execution_strategy {
        Some("torbox_http") => {
            if input.external_id.is_none_or(str::is_empty) {
                return Err("TorBox task missing remote ID");
            }
            match input.remote_state {
                Some("ready" | "completed") => Ok(RestartDecision::ResumeExistingRemote(
                    ResumeRemote::RefreshLink,
                )),
                Some("queued" | "cached" | "downloading" | "processing") | None => Ok(
                    RestartDecision::ResumeExistingRemote(ResumeRemote::PollExisting),
                ),
                Some(_) => Err("unknown TorBox remote state"),
            }
        }
        Some("local_qb") | None => Ok(RestartDecision::ResetQueued),
        Some(_) => Err("unknown execution strategy"),
    }
}
