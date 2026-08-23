use crate::domain::{EventKind, NetworkEvent, Process};

#[derive(Clone, Copy, PartialEq)]
pub enum ProcessTab {
    Applications,
    Processes,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DetailTab {
    Headers,
    RequestBody,
    Response,
    Timing,
    Connection,
}

impl DetailTab {
    pub const ALL: [Self; 5] = [
        Self::Headers,
        Self::RequestBody,
        Self::Response,
        Self::Timing,
        Self::Connection,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Headers => "Headers",
            Self::RequestBody => "Request body",
            Self::Response => "Response",
            Self::Timing => "Timing",
            Self::Connection => "Connection",
        }
    }
}

pub struct AppState {
    pub processes: Vec<Process>,
    pub events: Vec<NetworkEvent>,
    pub selected_pid: Option<u32>,
    pub selected_event: Option<u64>,
    pub detail_tab: DetailTab,
    pub filter: String,
    pub filter_error: Option<String>,
    pub https_only: bool,
    pub recording: bool,
    pub session_seconds: u64,
    pub last_tick: std::time::Instant,
    pub toast: Option<String>,
    pub capture_error: Option<String>,
    pub confirm_relaunch_all: bool,
    pub confirm_trust_certificate: bool,
    pub process_tab: ProcessTab,
}

impl AppState {
    pub fn new(processes: Vec<Process>, events: Vec<NetworkEvent>) -> Self {
        Self {
            processes,
            events,
            selected_pid: None,
            selected_event: None,
            detail_tab: DetailTab::Headers,
            filter: String::new(),
            filter_error: None,
            https_only: false,
            recording: true,
            session_seconds: 0,
            last_tick: std::time::Instant::now(),
            toast: None,
            capture_error: None,
            confirm_relaunch_all: false,
            confirm_trust_certificate: false,
            process_tab: ProcessTab::Applications,
        }
    }

    pub fn visible_event_indices(&self) -> Vec<usize> {
        if validate_filter(&self.filter).is_err() {
            return Vec::new();
        }

        self.events
            .iter()
            .enumerate()
            .filter(|(_, event)| self.event_matches_selected_process(event))
            .filter(|(_, event)| !self.https_only || event.kind == EventKind::Https)
            .filter(|(_, event)| matches_filter(event, &self.filter))
            .map(|(index, _)| index)
            .collect()
    }

    fn event_matches_selected_process(&self, event: &NetworkEvent) -> bool {
        let Some(pid) = self.selected_pid else {
            return true;
        };
        event.pid == Some(pid)
            || (self.process_tab == ProcessTab::Applications
                && self
                    .processes
                    .iter()
                    .find(|process| process.pid == pid)
                    .is_some_and(|process| event.process.eq_ignore_ascii_case(&process.name)))
    }

    pub fn selected_process_name(&self) -> String {
        self.selected_pid
            .and_then(|pid| self.processes.iter().find(|process| process.pid == pid))
            .map_or_else(
                || "All processes".into(),
                |process| format!("{} · PID {}", process.name, process.pid),
            )
    }

    pub fn tick(&mut self) {
        if self.recording {
            let elapsed = self.last_tick.elapsed().as_secs();
            if elapsed > 0 {
                self.session_seconds = self.session_seconds.saturating_add(elapsed);
                self.last_tick = std::time::Instant::now();
            }
        } else {
            self.last_tick = std::time::Instant::now();
        }
    }
}

pub fn validate_filter(filter: &str) -> Result<(), String> {
    for token in filter
        .split_whitespace()
        .filter(|token| token.contains(':'))
    {
        let (key, value) = token.split_once(':').unwrap_or_default();
        if !matches!(
            key.to_lowercase().as_str(),
            "process" | "host" | "method" | "status" | "protocol" | "ip" | "port"
        ) {
            return Err(format!(
                "Unknown filter ‘{key}’. Try host:, method:, protocol: or port:."
            ));
        }
        if matches!(key.to_lowercase().as_str(), "status" | "port") && value.parse::<u16>().is_err()
        {
            return Err(format!("‘{value}’ is not a valid {key} number."));
        }
    }
    Ok(())
}

fn matches_filter(event: &NetworkEvent, filter: &str) -> bool {
    if filter.trim().is_empty() {
        return true;
    }
    let searchable = event.search_text();
    filter.split_whitespace().all(|token| {
        let token = token.to_lowercase();
        let Some((key, value)) = token.split_once(':') else {
            return searchable.contains(&token);
        };
        match key {
            "process" => event.process.to_lowercase().contains(value),
            "host" => event.host.to_lowercase().contains(value),
            "method" => event.method.to_lowercase() == value,
            "status" => event
                .status
                .is_some_and(|status| status.to_string() == value),
            "protocol" => event.kind.label().to_lowercase().contains(value),
            "ip" => event.local.contains(value) || event.remote.contains(value),
            "port" => {
                event.local.ends_with(&format!(":{value}"))
                    || event.remote.ends_with(&format!(":{value}"))
            }
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EventKind;

    #[test]
    fn filter_combines_terms_and_reports_invalid_values() {
        let event = NetworkEvent {
            id: 1,
            pid: Some(42),
            process: "client.exe".into(),
            method: "GET".into(),
            host: "example.com".into(),
            path: "/".into(),
            status: Some(200),
            kind: EventKind::Https,
            bytes_sent: 100,
            bytes_received: 200,
            packet_count: 2,
            duration_ms: Some(10),
            local: "127.0.0.1:50000".into(),
            remote: "93.184.216.34:443".into(),
            request_headers: Vec::new(),
            response_headers: Vec::new(),
            request_body: None,
            response_body: None,
        };

        assert!(matches_filter(
            &event,
            "process:client.exe protocol:https port:443"
        ));
        assert!(!matches_filter(&event, "method:POST"));
        assert!(validate_filter("port:not-a-number").is_err());
        assert!(validate_filter("owner:me").is_err());

        let mut tcp_event = event.clone();
        tcp_event.id = 2;
        tcp_event.kind = EventKind::Tcp;
        let mut state = AppState::new(Vec::new(), vec![event, tcp_event]);
        state.https_only = true;
        assert_eq!(state.visible_event_indices(), vec![0]);
    }

    #[test]
    fn application_selection_includes_matching_child_processes() {
        let process = Process {
            pid: 42,
            name: "browser.exe".into(),
            active: true,
            application: true,
            icon_rgba: None,
        };
        let event = NetworkEvent {
            id: 1,
            pid: Some(99),
            process: "Browser.exe".into(),
            method: "TCP".into(),
            host: "example.com".into(),
            path: String::new(),
            status: None,
            kind: EventKind::Tcp,
            bytes_sent: 0,
            bytes_received: 0,
            packet_count: 1,
            duration_ms: None,
            local: "127.0.0.1:50000".into(),
            remote: "93.184.216.34:443".into(),
            request_headers: Vec::new(),
            response_headers: Vec::new(),
            request_body: None,
            response_body: None,
        };
        let mut state = AppState::new(vec![process], vec![event]);
        state.selected_pid = Some(42);

        assert_eq!(state.visible_event_indices(), vec![0]);
        state.process_tab = ProcessTab::Processes;
        assert!(state.visible_event_indices().is_empty());
    }
}
