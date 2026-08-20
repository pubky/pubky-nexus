use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

use super::processors::MediaProcessorError;

/// Wall-clock deadline for a single media subprocess.
///
/// Tool-level limits (ImageMagick's `MAGICK_TIME_LIMIT`, ffmpeg's `-timelimit`) bound CPU time, so a
/// child wedged on I/O trips none of them while holding its media permit for good. This is the
/// backstop for that case: set well above the tool budget, it only fires when a tool limit did not.
#[derive(Clone, Copy)]
pub struct MediaSubprocess {
    deadline: Duration,
}

impl MediaSubprocess {
    pub fn new(deadline: Duration) -> Self {
        Self { deadline }
    }

    /// Runs `command` to completion, killing and reaping it if it outruns the deadline.
    pub async fn run(&self, command: &mut Command) -> Result<Output, MediaProcessorError> {
        // stdin is closed rather than inherited: ffmpeg prompts on output collision and would
        // otherwise block on the server's stdin forever.
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(MediaProcessorError::command_failed)?;

        let mut stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| MediaProcessorError::command_failed("child stdout was not captured"))?;
        let mut stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| MediaProcessorError::command_failed("child stderr was not captured"))?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        // Both pipes are drained while waiting: a child that fills one while we wait on the other
        // deadlocks, and ffmpeg is chatty enough to reach the buffer limit.
        let wait = async {
            let (_, _, status) = tokio::try_join!(
                stdout_pipe.read_to_end(&mut stdout),
                stderr_pipe.read_to_end(&mut stderr),
                child.wait(),
            )?;
            Ok::<_, std::io::Error>(status)
        };

        // Its own statement so `wait`, and the borrow of `child` it holds, is dropped before the
        // timeout branch kills it.
        let outcome = tokio::time::timeout(self.deadline, wait).await;

        match outcome {
            Ok(status) => Ok(Output {
                status: status.map_err(MediaProcessorError::command_failed)?,
                stdout,
                stderr,
            }),
            Err(_elapsed) => {
                kill_and_reap(&mut child).await;
                Err(MediaProcessorError::Timeout {
                    command: command
                        .as_std()
                        .get_program()
                        .to_string_lossy()
                        .into_owned(),
                    deadline: self.deadline,
                })
            }
        }
    }
}

/// SIGKILLs the child, then reaps it so no zombie is left behind. A helper the child forked of its
/// own (ImageMagick links its SVG/HEIC/RAW coders in rather than forking) would outlive this; if one
/// ever does, `Command::process_group(0)` plus a group-wide kill is the answer.
async fn kill_and_reap(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use tokio::process::Command;

    use super::MediaSubprocess;
    use crate::media::processors::MediaProcessorError;

    #[tokio_shared_rt::test(shared)]
    async fn test_output_is_captured_on_success() {
        let runner = MediaSubprocess::new(Duration::from_secs(5));
        let output = runner
            .run(Command::new("echo").arg("hello"))
            .await
            .expect("echo must run");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    // A failing command is the callers' business to interpret, not the runner's error.
    #[tokio_shared_rt::test(shared)]
    async fn test_failing_command_returns_its_status() {
        let runner = MediaSubprocess::new(Duration::from_secs(5));
        let output = runner
            .run(&mut Command::new("false"))
            .await
            .expect("a non-zero exit is not a runner error");

        assert!(!output.status.success());
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_deadline_kills_and_reaps_the_child() {
        let runner = MediaSubprocess::new(Duration::from_millis(100));
        let started = Instant::now();
        let error = runner
            .run(Command::new("sleep").arg("30"))
            .await
            .expect_err("the child must not outlive the deadline");

        assert!(
            matches!(error, MediaProcessorError::Timeout { .. }),
            "expected a timeout, got {error}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the deadline must return without waiting for the child"
        );
    }

    // A permit is only worth reclaiming if the process it stood for is really gone, not merely
    // abandoned to keep working.
    #[tokio_shared_rt::test(shared)]
    async fn test_deadline_kills_the_child_rather_than_abandoning_it() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let marker = dir.path().join("child-kept-working");

        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(format!("sleep 1; touch {}", marker.to_string_lossy()));

        let runner = MediaSubprocess::new(Duration::from_millis(100));
        let error = runner.run(&mut command).await.expect_err("must time out");
        assert!(matches!(error, MediaProcessorError::Timeout { .. }));

        // Past when the child would have written it, had it survived.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(
            !marker.exists(),
            "the child outlived the deadline and kept working"
        );
    }
}
