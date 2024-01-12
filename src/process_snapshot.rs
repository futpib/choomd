use std::path::PathBuf;

#[derive(Debug)]
pub struct ProcessSnapshot {
    pub pid: i32,
    pub uid: u32,
    pub command_line: Vec<String>,
    pub current_working_directory: PathBuf,
    pub oom_score: i32,
    pub oom_score_adjust: i32,
}

impl ProcessSnapshot {
    pub fn command_line_file_path(&self) -> Option<String> {
        let command_line_filepath = self.command_line.first()?;
        Some(command_line_filepath.clone())
    }

    pub fn command_line_file_name(&self) -> Option<String> {
        let command_line_filepath = self.command_line_file_path()?;
        let command_line_file_name = PathBuf::from(command_line_filepath)
            .file_name()?
            .to_string_lossy()
            .to_string();
        Some(command_line_file_name)
    }

    pub fn command_line_arguments(&self) -> Vec<String> {
        let mut command_line_arguments = Vec::<String>::new();

        for command_line_argument in self.command_line.iter().skip(1) {
            command_line_arguments.push(command_line_argument.clone());
        }

        command_line_arguments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_line_getters() {
        let ok_process_snapshot = ProcessSnapshot {
            pid: 1,
            uid: 1,
            command_line: vec!["/bin/echo".to_string(), "hello".to_string()],
            current_working_directory: PathBuf::from("/"),
            oom_score: 1,
            oom_score_adjust: 1,
        };

        assert_eq!(
            ok_process_snapshot.command_line_file_path(),
            Some("/bin/echo".to_string())
        );

        assert_eq!(
            ok_process_snapshot.command_line_file_name(),
            Some("echo".to_string())
        );

        assert_eq!(
            ok_process_snapshot.command_line_arguments(),
            vec!["hello".to_string()]
        );

        let setproctitle_process_snapshot = ProcessSnapshot {
            pid: 1,
            uid: 1,
            command_line: vec!["I control my process title".to_string()],
            current_working_directory: PathBuf::from("/"),
            oom_score: 1,
            oom_score_adjust: 1,
        };

        assert_eq!(
            setproctitle_process_snapshot.command_line_file_path(),
            Some("I control my process title".to_string())
        );

        assert_eq!(
            setproctitle_process_snapshot.command_line_file_name(),
            Some("I control my process title".to_string())
        );

        assert_eq!(
            setproctitle_process_snapshot.command_line_arguments(),
            Vec::<String>::new()
        );
    }
}
