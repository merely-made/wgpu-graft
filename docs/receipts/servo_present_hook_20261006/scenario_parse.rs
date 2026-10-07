// Source-only grammar check using the actual Turnstone parser and qualified
// Inker library. This does not compile or qualify the upstream Servo host.
#[path = "../../../../turnstone/src/scenario.rs"]
mod scenario;

fn main() {
    let path = std::env::args().nth(1).expect("scenario path");
    let source = std::fs::read_to_string(&path).expect("scenario source");
    let expanded = source.lines().enumerate()
        .map(|(i, line)| scenario::expand_env(line, i + 1))
        .collect::<Result<Vec<_>, _>>().expect("environment expansion");
    taproot::Scenario::parse(&expanded.join("\n")).expect("actual shared Taproot grammar");
    let mut app_steps = 0;
    for line in expanded.iter().map(|line| line.trim()) {
        let verb = line.split_whitespace().next().unwrap_or("");
        let app_assert = verb == "assert" && !["assert snap ", "assert text ", "assert event "].iter().any(|prefix| line.starts_with(prefix));
        if app_assert || ["open", "key", "type", "click-at", "divider", "record-idle"].contains(&verb) {
            app_steps += scenario::parse(line).expect("actual Turnstone app-step grammar").len();
        }
    }
    println!("Shared Taproot grammar accepted {path}; {app_steps} app steps also parsed by actual Turnstone grammar");
}
