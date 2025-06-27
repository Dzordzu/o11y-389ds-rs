use serde::{Deserialize, Serialize};

#[derive(Copy, Debug, Clone, Serialize, Deserialize)]
enum Maintenance {
    Ready,
    Maint,
}

#[derive(Copy, Debug, Clone, Serialize, Deserialize)]
enum Status {
    Up,
    Down,
    Fail,
    Stopped,
    Maintenance(Maintenance),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    status: Status,
    pub maxxconn: Option<u64>,

    /// Weight in the %
    pub weight: Option<u64>,

    /// Reason of the stop, down or fail
    reason: Option<String>,
}

impl Response {
    pub fn drain(&mut self) -> &mut Self {
        self.weight = Some(0);
        self
    }

    pub fn maintenance(&mut self) -> &mut Self {
        self.status = Status::Maintenance(Maintenance::Maint);
        self
    }

    pub fn up_and_ready(&mut self) -> &mut Self {
        self.weight = Some(100);
        self.reason = None;
        if matches!(self.status, Status::Maintenance(Maintenance::Maint)) {
            self.status = Status::Maintenance(Maintenance::Ready)
        } else {
            self.status = Status::Up;
        }
        self
    }

    pub fn fail(&mut self, reason: Option<&str>) -> &mut Self {
        self.status = Status::Fail;
        self.reason = reason.map(String::from);
        self
    }

    pub fn stopped(&mut self, reason: Option<&str>) -> &mut Self {
        self.status = Status::Stopped;
        self.reason = reason.map(String::from);
        self
    }

    pub fn down(&mut self, reason: Option<&str>) -> &mut Self {
        self.status = Status::Down;
        self.reason = reason.map(String::from);
        self
    }

    pub fn to_haproxy_string(&self) -> String {
        let status_str = match self.status {
            Status::Up => "up",
            Status::Down => "down",
            Status::Fail => "fail",
            Status::Stopped => "stopped",
            Status::Maintenance(maintenance) => match maintenance {
                Maintenance::Ready => "ready",
                Maintenance::Maint => "maint",
            },
        };

        let reason_str = match self.status {
            Status::Fail | Status::Stopped | Status::Down => {
                let reason = self.reason.as_ref().map(|reason| {
                    format!(
                        " #{}",
                        reason
                            .trim()
                            .replace("\n", " ")
                            .chars()
                            .filter(|x| x.is_ascii())
                            .collect::<String>()
                    )
                });
                reason.unwrap_or_default()
            }
            _ => "".to_string(),
        };

        let maxconn_str = if matches!(
            self.status,
            Status::Up | Status::Maintenance(Maintenance::Ready)
        ) {
            match self.maxxconn {
                Some(maxxconn) => format!(" maxconn:{}", maxxconn),
                None => "".to_string(),
            }
        } else {
            String::default()
        };

        let weight_str = if matches!(
            self.status,
            Status::Up | Status::Maintenance(Maintenance::Ready)
        ) {
            match self.weight {
                Some(weight) => format!(" weight:{}%", weight),
                None => "".to_string(),
            }
        } else {
            String::default()
        };

        format!("{status_str}{weight_str}{maxconn_str}{reason_str}\n",)
    }

    pub fn new_up() -> Self {
        Response {
            status: Status::Up,
            maxxconn: None,
            weight: None,
            reason: None,
        }
    }

    pub fn new_down() -> Self {
        Response {
            status: Status::Down,
            maxxconn: None,
            weight: None,
            reason: None,
        }
    }
}
