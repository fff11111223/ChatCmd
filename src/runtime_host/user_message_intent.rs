use serde_json::{Value, json};

pub(super) fn is_plan_mode_request(content: &str) -> bool {
    let normalized = intent_prose(content)
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let negated = [
        "không cần lên kế hoạch",
        "không cần lập kế hoạch",
        "không lên kế hoạch",
        "không lập kế hoạch",
        "do not make a plan",
        "do not make a plan",
        "do not plan",
        "don't plan",
        "no plan needed",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase));
    !negated
        && (normalized.contains("lên kế hoạch")
            || normalized.contains("lập kế hoạch")
            || normalized.split_whitespace().any(|word| word == "#plan"))
}

pub(super) fn is_explicit_multi_agent_request(content: &str) -> bool {
    let normalized = intent_prose(content)
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let negated = [
        "không chia agent",
        "không cần chia agent",
        "do not split agents",
        "không thử chia agent",
        "không cần thử chia agent",
        "do not try to split agents",
        "không muốn chia agent",
        "không dùng nhiều agent",
        "không sử dụng nhiều agent",
        "do not split into agents",
        "do not try to split into agents",
        "don't try to split into agents",
        "don't split into agents",
        "do not use multiple agents",
        "don't use multiple agents",
        "without subagents",
        "no subagents",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase));
    if negated {
        return false;
    }

    let starts_with_command = |command: &str| {
        normalized.strip_prefix(command).is_some_and(|rest| {
            rest.is_empty()
                || rest.starts_with(' ')
                || rest.starts_with(':')
                || rest.starts_with('-')
        })
    };

    starts_with_command("chia agent")
        || ((starts_with_command("chia ra") || starts_with_command("tách ra"))
            && normalized.contains(" agent"))
        || starts_with_command("dùng nhiều agent")
        || starts_with_command("sử dụng nhiều agent")
        || starts_with_command("use multiple agents")
        || starts_with_command("use several agents")
        || starts_with_command("split into agents")
        || starts_with_command("split across agents")
        || [
            "hãy chia agent",
            "vui lòng chia agent",
            "thử chia agent",
            "giúp tôi chia agent",
            "giúp t chia agent",
            "hãy dùng nhiều agent",
            "vui lòng dùng nhiều agent",
            "hãy sử dụng nhiều agent",
            "please use multiple agents",
            "please split into agents",
        ]
        .iter()
        .any(|phrase| normalized.contains(phrase))
}

pub(super) fn intent_hint(content: &str) -> Value {
    let normalized = intent_prose(content).to_lowercase();
    let workflow_kind = if is_plan_mode_request(content) {
        "plan"
    } else if [
        "chỉ review",
        "only evaluate",
        "review only",
        "do not edit",
        "do not modify",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase))
    {
        "review"
    } else if ["debug", "sửa lỗi", "fix bug"]
        .iter()
        .any(|phrase| normalized.contains(phrase))
    {
        "debug"
    } else if ["commit", "tạo commit"]
        .iter()
        .any(|phrase| normalized.contains(phrase))
    {
        "commit"
    } else {
        "implement"
    };
    json!({
        "workflowKind": workflow_kind,
        "authoritative": false,
        "grantsExecutionPermission": false,
        "note": "Language classification is a workflow hint only; effective effects come from task policy and authenticated user decisions."
    })
}

fn intent_prose(content: &str) -> String {
    let mut prose = String::with_capacity(content.len());
    let mut fenced = false;
    for line in content.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut delimiter = None;
        for character in line.chars() {
            if matches!(character, '`' | '"' | '\'') {
                delimiter = match delimiter {
                    Some(active) if active == character => None,
                    None => Some(character),
                    active => active,
                };
            } else if delimiter.is_none() {
                prose.push(character);
            }
        }
        prose.push(' ');
    }
    prose
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_trigger_ignores_negation_quotes_and_code() {
        assert!(is_plan_mode_request("Lên kế hoạch cho tôi mua quà"));
        assert!(is_plan_mode_request("MAKE   A PLAN\\nsales website"));
        assert!(is_plan_mode_request("Xây website giúp tôi #PLAN"));
        assert!(!is_plan_mode_request("Cho tôi xem kế hoạch hiện tại"));
        assert!(!is_plan_mode_request("Use the planner to track the work"));
        assert!(!is_plan_mode_request("Không cần lên kế hoạch, sửa luôn"));
        assert!(!is_plan_mode_request(
            "Do not make a plan; implement directly"
        ));
        assert!(!is_plan_mode_request("Log ghi `#plan` nhưng hãy sửa lỗi"));
        assert!(!is_plan_mode_request(
            "Ví dụ:\n```text\nlập kế hoạch\n```\nSửa code"
        ));
        assert!(!is_plan_mode_request("Review chuỗi \"lên kế hoạch\""));
    }

    #[test]
    fn multi_agent_intent_requires_an_explicit_user_request() {
        assert!(is_explicit_multi_agent_request(
            "Split an agent to read files for me"
        ));
        assert!(is_explicit_multi_agent_request(
            "Chia agent: create delegated reviewer"
        ));
        assert!(is_explicit_multi_agent_request(
            "Try splitting an agent to read the uncommitted files"
        ));
        assert!(is_explicit_multi_agent_request(
            "Split into 3 agents to audit in parallel"
        ));
        assert!(is_explicit_multi_agent_request(
            "Use multiple agents to review this repo"
        ));
        assert!(!is_explicit_multi_agent_request(
            "Review the entire source code and subagents to find errors"
        ));
        assert!(!is_explicit_multi_agent_request(
            "Kiểm tra logic chia agent hiện tại"
        ));
        assert!(!is_explicit_multi_agent_request(
            "Kiểm tra logic dùng nhiều agent hiện tại"
        ));
        assert!(!is_explicit_multi_agent_request(
            "Không chia agent, làm trong cuộc trò chuyện hiện tại"
        ));
        assert!(!is_explicit_multi_agent_request(
            "Review chuỗi `chia agent` trong source"
        ));
        assert!(!is_explicit_multi_agent_request(
            "Do not try to split agents; work in the current conversation"
        ));
        assert!(!is_explicit_multi_agent_request(
            "Review nhãn \"thử chia agent\" trong UI"
        ));
    }

    #[test]
    fn intent_hint_never_grants_execution_permission() {
        let review = intent_hint("Chỉ review, do not modify");
        assert_eq!(review["workflowKind"], "review");
        assert_eq!(review["authoritative"], false);
        assert_eq!(review["grantsExecutionPermission"], false);
    }
}
