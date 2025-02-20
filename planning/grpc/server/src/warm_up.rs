use aries_planning::chronicles::ChronicleTemplate;

pub fn depth_from_option_plan(plan: Option<String>, ch: &ChronicleTemplate) -> u32 {
    plan.map_or(0, |p| depth_from_plan(p, ch))
}

pub fn depth_from_plan(plan: String, ch: &ChronicleTemplate) -> u32 {
    let ch_name = format!("{:?}", ch.label);

    plan.split("\n")
        .map(|line| line.trim())
        .filter(|line| line.starts_with(ch_name.as_str()))
        .count() as u32
}
