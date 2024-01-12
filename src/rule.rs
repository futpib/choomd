use crate::process_snapshot::ProcessSnapshot;

#[derive(Debug)]
pub struct Rule {
    pub key: String,
    pub command_line_file_path: Vec<String>,
    pub command_line_file_name: Vec<String>,
    pub command_line_argument: Vec<String>,
    pub current_working_directory: Vec<String>,
    pub owner_user_id: Vec<u32>,
    pub oom_score_adj: i32,
}

fn rule_matches_generic<T: Eq>(rule_values: &Vec<T>, process_snapshot_values: &Vec<T>, predicate: fn(&T, &T) -> bool) -> bool {
    if rule_values.is_empty() {
        return true;
    }

    if process_snapshot_values.is_empty() {
        return false;
    }

    rule_values.iter().any(|rule_value| {
        process_snapshot_values
            .iter()
            .any(|process_snapshot_value| predicate(process_snapshot_value, rule_value))
    })
}

fn rule_matches_generic_glob(rule_values: &Vec<String>, process_snapshot_values: &Vec<String>) -> bool {
    rule_matches_generic(rule_values, process_snapshot_values, |process_snapshot_value, rule_value| {
        glob::Pattern::new(rule_value).unwrap().matches(process_snapshot_value)
    })
}

fn rule_matches_generic_eq<T: Eq>(rule_values: &Vec<T>, process_snapshot_values: &Vec<T>) -> bool {
    rule_matches_generic(rule_values, process_snapshot_values, |process_snapshot_value, rule_value| {
        process_snapshot_value == rule_value
    })
}

impl Rule {
    pub fn merge(&self, other: &Rule) -> Rule {
        let command_line_file_path = if self.command_line_file_path.is_empty() {
            other.command_line_file_path.clone()
        } else {
            self.command_line_file_path.clone()
        };

        let command_line_file_name = if self.command_line_file_name.is_empty() {
            other.command_line_file_name.clone()
        } else {
            self.command_line_file_name.clone()
        };

        let command_line_argument = if self.command_line_argument.is_empty() {
            other.command_line_argument.clone()
        } else {
            self.command_line_argument.clone()
        };

        let current_working_directory = if self.current_working_directory.is_empty() {
            other.current_working_directory.clone()
        } else {
            self.current_working_directory.clone()
        };

        let owner_user_id = if self.owner_user_id.is_empty() {
            other.owner_user_id.clone()
        } else {
            self.owner_user_id.clone()
        };

        let oom_score_adj = if self.oom_score_adj == 0 {
            other.oom_score_adj
        } else {
            self.oom_score_adj
        };

        Rule {
            key: self.key.clone(),
            command_line_file_path,
            command_line_file_name,
            command_line_argument,
            current_working_directory,
            owner_user_id,
            oom_score_adj,
        }
    }

    fn matches_command_line_file_path(&self, process_snapshot: &ProcessSnapshot) -> bool {
        rule_matches_generic_glob(
            &self.command_line_file_path,
            &if let Some(file_path) = process_snapshot.command_line_file_path() {
                vec![file_path]
            } else {
                vec![]
            },
        )
    }

    fn matches_command_line_file_name(&self, process_snapshot: &ProcessSnapshot) -> bool {
        rule_matches_generic_glob(
            &self.command_line_file_name,
            &if let Some(file_name) = process_snapshot.command_line_file_name() {
                vec![file_name]
            } else {
                vec![]
            },
        )
    }

    fn matches_command_line_argument(&self, process_snapshot: &ProcessSnapshot) -> bool {
        rule_matches_generic_glob(
            &self.command_line_argument,
            &process_snapshot.command_line_arguments(),
        )
    }

    fn matches_current_working_directory(&self, process_snapshot: &ProcessSnapshot) -> bool {
        rule_matches_generic_glob(
            &self.current_working_directory,
            &vec![process_snapshot.current_working_directory.to_string_lossy().to_string()],
        )
    }

    fn matches_owner_user(&self, process_snapshot: &ProcessSnapshot) -> bool {
        rule_matches_generic_eq(&self.owner_user_id, &vec![process_snapshot.uid])
    }

    pub fn matches(&self, process_snapshot: &ProcessSnapshot) -> bool {
        true && self.matches_command_line_file_path(process_snapshot)
            && self.matches_command_line_file_name(process_snapshot)
            && self.matches_command_line_argument(process_snapshot)
            && self.matches_current_working_directory(process_snapshot)
            && self.matches_owner_user(process_snapshot)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_glob_match() {
        let tsserver_process_snapshot = ProcessSnapshot {
            pid: 1,
            uid: 1,
            command_line: "/usr/bin/node /home/futpib/code/whatever/node_modules/typescript/lib/tsserver.js".split(" ").map(|s| s.to_string()).collect(),
            current_working_directory: PathBuf::from("/home/futpib/code/whatever"),
            oom_score: 1,
            oom_score_adjust: 1,
        };

        let tsserver_rule = Rule {
            key: "tsserver".to_string(),
            command_line_file_path: vec![],
            command_line_file_name: vec!["node".to_string()],
            command_line_argument: vec!["**/tsserver.js".to_string()],
            current_working_directory: vec![],
            owner_user_id: vec![],
            oom_score_adj: 1,
        };

        assert!(tsserver_rule.matches(&tsserver_process_snapshot));
    }
}
