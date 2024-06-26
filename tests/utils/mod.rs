use cuid::cuid2;
use std::default::Default;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::process::{Command, Stdio};
use std::time::SystemTime;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

pub struct Quartz {
    bin: PathBuf,
    tmpdir: PathBuf,
}

pub struct QuartzOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: ExitStatus,
}

impl Default for Quartz {
    fn default() -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        let tmpdir = std::env::temp_dir()
            .join("quartz_cli_tests")
            .join(now.as_millis().to_string())
            .join(cuid2());
        let bin = Path::new(env!("CARGO_BIN_EXE_quartz")).to_path_buf();

        std::fs::create_dir_all(&tmpdir).unwrap();

        Quartz { tmpdir, bin }
    }
}

impl Drop for Quartz {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.dir()).unwrap();
    }
}

impl Quartz {
    pub fn preset_empty_project() -> Result<Self, std::io::Error> {
        let quartz = Quartz::default();
        quartz.cmd(&["init"])?;

        Ok(quartz)
    }

    pub fn preset_using_sample_endpoint() -> Result<Self, std::io::Error> {
        let quartz = Quartz::preset_empty_project()?;
        let sample_endpoint = "myendpoint";

        quartz.cmd(&[
            "create",
            sample_endpoint,
            "--url",
            "https://httpbin.org/get",
        ])?;

        quartz.cmd(&["use", sample_endpoint])?;

        Ok(quartz)
    }

    /// httpbin.org mapping.
    ///
    /// # Endpoints
    ///
    /// * httpbin/get
    /// * httpbin/post
    /// * httpbin/redirect
    /// * httpbin/redirect/absolute
    /// * httpbin/redirect/relative
    pub fn preset_httpbin() -> Result<Self, std::io::Error> {
        let quartz = Quartz::preset_empty_project()?;

        quartz.cmd(&["var", "set", "BASE_URL=http://httpbin.org"])?;

        quartz.cmd(&[
            "create",
            "httpbin/get",
            "--url",
            "{{BASE_URL}}/get",
            "-q",
            "value=10",
        ])?;

        quartz.cmd(&[
            "create",
            "httpbin/post",
            "-X",
            "POST",
            "--url",
            "{{BASE_URL}}/post",
        ])?;

        quartz.cmd(&[
            "create",
            "httpbin/redirect",
            "--url",
            "{{BASE_URL}}/redirect/{{N}}",
        ])?;
        quartz.cmd(&[
            "create",
            "httpbin/redirect/absolute",
            "--url",
            "{{BASE_URL}}/absolute-redirect/{{N}}",
        ])?;
        quartz.cmd(&[
            "create",
            "httpbin/redirect/relative",
            "--url",
            "{{BASE_URL}}/relative-redirect/{{N}}",
        ])?;

        quartz.cmd(&["use", "httpbin/get"])?;

        Ok(quartz)
    }

    pub fn preset_using_default_env() -> Result<Self, std::io::Error> {
        let quartz = Quartz::preset_empty_project()?;

        quartz.cmd(&["env", "use", "default"])?;

        Ok(quartz)
    }

    pub fn cmd<S>(&self, args: &[S]) -> Result<QuartzOutput, std::io::Error>
    where
        S: AsRef<OsStr>,
    {
        let output = Command::new(self.bin.as_path())
            .current_dir(self.tmpdir.as_path())
            .args(args)
            .env("NO_COLOR", "1")
            .output()?;

        Ok(QuartzOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
            status: output.status,
        })
    }

    pub fn cmd_stdin<S>(&self, args: &[S], stdin: &str) -> Result<QuartzOutput, std::io::Error>
    where
        S: AsRef<OsStr>,
    {
        let mut child = Command::new(self.bin.as_path())
            .current_dir(self.tmpdir.as_path())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let child_stdin = child.stdin.as_mut().unwrap();
        child_stdin.write_all(stdin.as_bytes())?;

        let output = child.wait_with_output()?;

        Ok(QuartzOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into(),
            stderr: String::from_utf8_lossy(&output.stderr).into(),
            status: output.status,
        })
    }

    pub fn dir(&self) -> PathBuf {
        self.tmpdir.join(".quartz")
    }
}
