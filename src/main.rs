use std::{path::PathBuf, thread::sleep, time::Duration, convert::TryInto};

use humantime::parse_duration;
use procfs::process::all_processes;

use clap::Parser;
use users::get_user_by_name;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the config file
    #[arg(long, default_value = "/etc/choomd.toml")]
    config_file: String,

    /// Print processes with oom scores and exit
    #[arg(long)]
    top: bool,
}

#[derive(Debug)]
struct ProcessSnapshot {
    pid: i32,
    uid: u32,
    command_line: Vec<String>,
    oom_score: i32,
    oom_score_adjust: i32,
}

impl ProcessSnapshot {
    fn command_line_file_name(&self) -> Option<String> {
        let command_line_filepath = self.command_line.first()?;
        let command_line_file_name = PathBuf::from(command_line_filepath)
            .file_name()?
            .to_string_lossy()
            .to_string();
        Some(command_line_file_name)
    }
}

#[derive(Debug)]
struct Rule {
    command_line_file_name: Vec<String>,
    owner_user_id: Vec<u32>,
    badness_score: i32,
}

impl Rule {
    fn merge(&self, other: &Rule) -> Rule {
        let command_line_file_name = if self.command_line_file_name.is_empty() {
            other.command_line_file_name.clone()
        } else {
            self.command_line_file_name.clone()
        };

        let owner_user_id = if self.owner_user_id.is_empty() {
            other.owner_user_id.clone()
        } else {
            self.owner_user_id.clone()
        };

        let badness_score = if self.badness_score == 0 {
            other.badness_score
        } else {
            self.badness_score
        };

        Rule {
            command_line_file_name,
            owner_user_id,
            badness_score,
        }
    }

    fn matches_command_line_file_name(&self, process_snapshot: &ProcessSnapshot) -> bool {
        if self.command_line_file_name.is_empty() {
            return true;
        }

        let command_line_file_name = process_snapshot.command_line_file_name();

        if command_line_file_name.is_none() {
            return false;
        }

        let command_line_file_name = command_line_file_name.unwrap();

        for rule_command_line_file_name in self.command_line_file_name.iter() {
            if command_line_file_name.contains(rule_command_line_file_name) {
                return true;
            }
        }

        false
    }

    fn matches_owner_user(&self, process_snapshot: &ProcessSnapshot) -> bool {
        if self.owner_user_id.is_empty() {
            return true;
        }

        let owner_user_id = process_snapshot.uid;

        for rule_owner_user_id in self.owner_user_id.iter() {
            if owner_user_id == *rule_owner_user_id {
                return true;
            }
        }

        false
    }

    fn matches(&self, process_snapshot: &ProcessSnapshot) -> bool {
        self.matches_command_line_file_name(process_snapshot)
            && self.matches_owner_user(process_snapshot)
    }
}

const MIN_OOM_SCORE: i32 = 0;
const MAX_OOM_SCORE: i32 = 1000;

const MIN_OOM_SCORE_ADJ: i32 = -1000;
const MAX_OOM_SCORE_ADJ: i32 = 1000;

fn get_process_snapshots() -> Vec<ProcessSnapshot> {
    let mut process_snapshots = Vec::<ProcessSnapshot>::new();

    for process in all_processes().expect("Can't read /proc") {
        if let Err(_) = process {
            continue;
        }

        let process = process.unwrap();

        let process_snapshot = ProcessSnapshot {
            pid: process.pid,
            uid: process.uid()
                .unwrap_or_default(),
            command_line: process.cmdline()
                .unwrap_or_default(),
            oom_score: process.oom_score()
                .map(|oom_score| oom_score.into())
                .unwrap_or(MIN_OOM_SCORE),
            oom_score_adjust: process.oom_score_adj()
                .map(|oom_score_adj| oom_score_adj.into())
                .unwrap_or(MIN_OOM_SCORE_ADJ),
        };

        process_snapshots.push(process_snapshot);
    }

    process_snapshots.sort_by(|a, b| a.oom_score.cmp(&b.oom_score));

    process_snapshots
}

fn main_loop(
    poll_interval: Duration,
    rules: Vec<Rule>,
) {
    loop {
        let process_snapshots = get_process_snapshots();

        println!("{:#?}", process_snapshots);

        sleep(poll_interval);
    }
}

fn parse_rule(
    table: &toml::value::Table,
) -> Rule {
    let command_line_file_name = table.get("command_line_file_name")
        .and_then(|v| v.as_array())
        .map(|v| v.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect::<Vec<String>>())
        .unwrap_or_default();

    let owner_user_name = table.get("owner_user_name")
        .and_then(|v| v.as_array())
        .map(|v| v.iter().map(|v| v.as_str().unwrap_or("").to_string()).collect::<Vec<String>>())
        .unwrap_or_default();

    let owner_user_id = owner_user_name.iter()
        .map(|owner_user_name| {
            get_user_by_name(owner_user_name)
                .map(|user| user.uid())
                .expect(format!("Can't find user: {}", owner_user_name).as_str())
        })
        .collect::<Vec<u32>>();

    let badness_score = table.get("badness_score")
        .and_then(|v| v.as_integer())
        .map(|v| v.try_into().unwrap_or(0))
        .unwrap_or(0);

    let rule = Rule {
        command_line_file_name,
        owner_user_id,
        badness_score,
    };

    rule
}

fn parse_rules(
    table: &toml::value::Table,
) -> Vec<Rule> {
    let mut rules = Vec::<Rule>::new();

    let default_rule = parse_rule(table.get("default").and_then(|v| v.as_table()).unwrap_or(&toml::value::Table::new()));

    for (_key, value) in table {
        let rule = parse_rule(value.as_table().unwrap_or(&toml::value::Table::new()));

        rules.push(rule.merge(&default_rule));
    }

    rules.sort_by(|a, b| a.badness_score.cmp(&b.badness_score));

    rules
}

fn main_top() {
    let process_snapshots = get_process_snapshots();

    println!("OOM_SCORE OOM_SCORE_ADJ PID UID COMMAND_LINE");

    for process_snapshot in process_snapshots {
        println!("{} {} {} {} {}",
            process_snapshot.oom_score,
            process_snapshot.oom_score_adjust,
            process_snapshot.pid,
            process_snapshot.uid,
            process_snapshot.command_line.join(" "),
        );
    }
}

fn main() {
    let args = Args::parse();

    if (args.top) {
        main_top();
        return;
    }

    let config_path = PathBuf::from(args.config_file);
    let config_string = std::fs::read_to_string(config_path.clone())
        .expect(format!("Can't read config file: {}", config_path.to_string_lossy()).as_str());
    let config = toml::from_str::<toml::Value>(&config_string).expect("Can't parse config file");

    println!("{:?}", config);

    let poll_interval_string = config.get("poll_interval").and_then(|v| v.as_str()).unwrap_or("10s");
    let poll_interval = parse_duration(poll_interval_string).expect("Can't parse poll interval");

    let min_oom_score_adjust: i32 = config.get("min_oom_score_adjust")
        .and_then(|v| v.as_integer().map(|v| v.try_into().unwrap_or(MIN_OOM_SCORE_ADJ)))
        .unwrap_or(MIN_OOM_SCORE_ADJ.into());
    let max_oom_score_adjust: i32 = config.get("max_oom_score_adjust")
        .and_then(|v| v.as_integer().map(|v| v.try_into().unwrap_or(MAX_OOM_SCORE_ADJ)))
        .unwrap_or(MAX_OOM_SCORE_ADJ.into());

    let rules = parse_rules(config.get("rules").and_then(|v| v.as_table()).unwrap_or(&toml::value::Table::new()));

    println!("{:?}", rules);

    main_loop(
        poll_interval,
        rules,
    );
}
