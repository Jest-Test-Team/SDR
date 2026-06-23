use protocol::frame::{Payload, TelemetryFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOutcome {
    ActionTriggered,
    Logged,
}

pub fn evaluate(frame: &TelemetryFrame) -> RuleOutcome {
    match frame.payload {
        Payload::BoolCmd(true) => RuleOutcome::ActionTriggered,
        Payload::BoolCmd(false) => RuleOutcome::Logged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::frame::Payload;

    #[test]
    fn true_triggers_action() {
        let frame = TelemetryFrame {
            seq: 1,
            timestamp_ms: 0,
            node_id: 1,
            payload: Payload::BoolCmd(true),
        };
        assert_eq!(evaluate(&frame), RuleOutcome::ActionTriggered);
    }

    #[test]
    fn false_only_logs() {
        let frame = TelemetryFrame {
            seq: 1,
            timestamp_ms: 0,
            node_id: 1,
            payload: Payload::BoolCmd(false),
        };
        assert_eq!(evaluate(&frame), RuleOutcome::Logged);
    }
}
