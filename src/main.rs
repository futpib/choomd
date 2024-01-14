pub mod process_snapshot;
pub mod rule;

use std::{
    collections::BTreeSet, convert::TryInto, iter::FromIterator, path::PathBuf, process::exit,
    thread::sleep, time::Duration,
};

use humantime::parse_duration;
use log::{debug, error};
use rule::Rule;
use process_snapshot::ProcessSnapshot;
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
    ps: bool,
}

const MIN_OOM_SCORE: i32 = 0;
const MIN_OOM_SCORE_ADJ: i32 = -1000;

fn get_process_snapshots() -> Vec<ProcessSnapshot> {
    let mut process_snapshots = Vec::<ProcessSnapshot>::new();

    for process in all_processes().expect("Can't read /proc") {
        if let Err(_) = process {
            continue;
        }

        let process = process.unwrap();

        let process_snapshot = ProcessSnapshot {
            pid: process.pid,
            uid: process.uid().unwrap_or_default(),
            command_line: process.cmdline().unwrap_or_default(),
            current_working_directory: process.cwd().unwrap_or(PathBuf::from("/")),
            oom_score: process
                .oom_score()
                .map(|oom_score| oom_score.into())
                .unwrap_or(MIN_OOM_SCORE),
            oom_score_adjust: process
                .oom_score_adj()
                .map(|oom_score_adj| oom_score_adj.into())
                .unwrap_or(MIN_OOM_SCORE_ADJ),
        };

        process_snapshots.push(process_snapshot);
    }

    process_snapshots
}

fn main_loop(poll_interval: Duration, rules: Vec<Rule>) {
    loop {
        let process_snapshots = get_process_snapshots();
        let mut skipped_process_ids = BTreeSet::<i32>::new();

        for process_snapshot in process_snapshots {
            let matching_rule = rules.iter().find(|rule| rule.matches(&process_snapshot));

            if matching_rule.is_none() {
                continue;
            }

            let matching_rule = matching_rule.unwrap();
            let new_oom_score_adj = matching_rule.oom_score_adj;

            if process_snapshot.oom_score_adjust == new_oom_score_adj {
                skipped_process_ids.insert(process_snapshot.pid);

                continue;
            }

            debug!(
                "Setting oom_score_adj {} for process {}",
                new_oom_score_adj, process_snapshot.pid
            );

            if let Err(e) =
                procfs::process::Process::new(process_snapshot.pid).and_then(|process| {
                    process.set_oom_score_adj(
                        new_oom_score_adj
                            .try_into()
                            .expect("oom_score_adj out of range"),
                    )
                })
            {
                error!("Can't set oom_score_adj: {}", e);
            } else {
                println!(
                    "Set oom_score_adj {} (was {}) for process {} as per rule {}",
                    new_oom_score_adj,
                    process_snapshot.oom_score_adjust,
                    process_snapshot.pid,
                    matching_rule.key,
                );
            }
        }

        if !skipped_process_ids.is_empty() {
            debug!(
                "Skipped process ids (they already have expected oom_score_adj): {:?}",
                skipped_process_ids
            );
        }

        sleep(poll_interval);
    }
}

fn parse_rule_string_vec(table: &toml::value::Table, key: &str) -> Vec<String> {
    table
        .get(key)
        .and_then(|v| v.as_array())
        .map(|v| {
            v.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

fn parse_rule_u32_vec(table: &toml::value::Table, key: &str) -> Vec<u32> {
    table
        .get(key)
        .and_then(|v| v.as_array())
        .map(|v| {
            v.iter()
                .map(|v| {
                    v.as_integer()
                        .expect(format!("Can't parse {} as integer", key).as_str())
                        .try_into()
                        .expect(format!("Can't parse {} as u32", key).as_str())
                })
                .collect::<Vec<u32>>()
        })
        .unwrap_or_default()
}

fn parse_rule(key: &str, table: &toml::value::Table) -> Rule {
    let command_line_file_path = parse_rule_string_vec(table, "command_line_file_path");
    let command_line_file_name = parse_rule_string_vec(table, "command_line_file_name");
    let command_line_argument = parse_rule_string_vec(table, "command_line_argument");
    let current_working_directory = parse_rule_string_vec(table, "current_working_directory");
    let owner_user_name = parse_rule_string_vec(table, "owner_user_name");
    let owner_user_id = parse_rule_u32_vec(table, "owner_user_id");

    let owner_user_id_from_name: Vec<u32> = owner_user_name
        .iter()
        .map(|owner_user_name| {
            get_user_by_name(owner_user_name)
                .map(|user| user.uid())
                .expect(format!("Can't find user: {}", owner_user_name).as_str())
        })
        .collect::<Vec<u32>>();

    let owner_user_id = BTreeSet::from_iter(
        owner_user_id
            .into_iter()
            .chain(owner_user_id_from_name.into_iter()),
    )
    .into_iter()
    .collect::<Vec<u32>>();

    let oom_score_adj = table
        .get("oom_score_adj")
        .and_then(|v| v.as_integer())
        .map(|v| v.try_into().unwrap_or(0))
        .unwrap_or(0);

    let rule = Rule {
        key: key.to_string(),
        command_line_file_path,
        command_line_file_name,
        command_line_argument,
        current_working_directory,
        owner_user_id,
        oom_score_adj,
    };

    rule
}

fn is_uppercase(s: &str) -> bool {
    s.to_uppercase() == s
}

fn parse_rules(table: &toml::value::Table) -> Vec<Rule> {
    let mut rules = Vec::<Rule>::new();

    let default_rule = parse_rule(
        "DEFAULT",
        table
            .get("DEFAULT")
            .and_then(|v| v.as_table())
            .unwrap_or(&toml::value::Table::new()),
    );

    for (key, value) in table {
        if is_uppercase(key) {
            continue;
        }

        let rule = parse_rule(key, value.as_table().unwrap_or(&toml::value::Table::new()));

        rules.push(rule.merge(&default_rule));
    }

    rules
}

fn main_ps() {
    let process_snapshots = get_process_snapshots();

    println!("OOM_SCORE OOM_SCORE_ADJ PID UID CURRENT_WORKING_DIRECTORY COMMAND_LINE");

    for process_snapshot in process_snapshots {
        println!(
            "{} {} {} {} {} {}",
            process_snapshot.oom_score,
            process_snapshot.oom_score_adjust,
            process_snapshot.pid,
            process_snapshot.uid,
            process_snapshot.current_working_directory.to_string_lossy(),
            process_snapshot.command_line.join(" "),
        );
    }
}

fn main() {
    env_logger::init();

    let args = Args::parse();

    if args.ps {
        main_ps();
        return;
    }

    let config_path = PathBuf::from(args.config_file);
    let config_string = std::fs::read_to_string(config_path.clone())
        .expect(format!("Can't read config file: {}", config_path.to_string_lossy()).as_str());
    let config = toml::from_str::<toml::Value>(&config_string).expect("Can't parse config file");

    let poll_interval_string = config
        .get("poll_interval")
        .and_then(|v| v.as_str())
        .unwrap_or("10s");
    let poll_interval = parse_duration(poll_interval_string).expect("Can't parse poll interval");

    let rules = parse_rules(
        config
            .get("rules")
            .and_then(|v| v.as_table())
            .unwrap_or(&toml::value::Table::new()),
    );

    debug!("Rules dump:");
    debug!("OOM_SCORE_ADJ COMMAND_LINE_FILE_PATH COMMAND_LINE_FILE_NAME COMMAND_LINE_ARGUMENT OWNER_USER_ID");
    for rule in rules.iter() {
        debug!(
            "{} {:?} {:?} {:?} {:?}",
            rule.oom_score_adj,
            rule.command_line_file_path,
            rule.command_line_file_name,
            rule.command_line_argument,
            rule.owner_user_id
        );
    }

    if rules.is_empty() {
        error!(
            "No rules defined in config file {}, there is nothing to do.",
            config_path.to_string_lossy()
        );

        exit(1);
    }

    main_loop(poll_interval, rules);
}
