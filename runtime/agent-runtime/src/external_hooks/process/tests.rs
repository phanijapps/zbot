use super::*;
use crate::external_hooks::*;
#[cfg(target_os = "linux")]
use std::{
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

fn vault() -> (tempfile::TempDir, VaultPaths) {
    let temp = tempfile::tempdir().unwrap();
    let paths = VaultPaths::new(temp.path().to_owned());
    paths.ensure_dirs_exist().unwrap();
    std::fs::create_dir_all(paths.config_dir().join("hooks")).unwrap();
    (temp, paths)
}
#[cfg(target_os = "linux")]
fn managed_file(path: &Path, contents: &str, mode: u32) {
    std::fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
}
#[cfg(target_os = "linux")]
fn script(paths: &VaultPaths, contents: &str) -> PathBuf {
    let path = paths.config_dir().join("hooks/test.py");
    managed_file(&path, contents, 0o600);
    path
}
fn definition(command: Vec<String>) -> HookDefinition {
    HookDefinition {
        id: "fixture".into(),
        enabled: true,
        event: HookEvent::RunStart,
        command,
        cwd: HookCwd::Vault,
        timeout_ms: 1000,
        on_failure: HookFailurePolicy::Continue,
    }
}
fn event() -> HookEventPayload {
    HookEventPayload::new(
        "inv-1".into(),
        "sess-1".into(),
        "root".into(),
        HookEventData::RunStart {
            mode: HookMode::Chat,
        },
    )
}
async fn run(hook: &HookDefinition, paths: &VaultPaths) -> HookOutcome {
    invoke_hook(
        hook,
        paths,
        &[paths.wards_dir()],
        &event(),
        &[],
        &AtomicBool::new(false),
    )
    .await
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn python_observes_literal_argv_vault_eof_and_minimal_environment() {
    let (_temp, paths) = vault();
    let literal = "$HOME;$(touch should-not-exist)";
    let code=format!("import json,sys,os\nevent=json.load(sys.stdin)\nassert sys.argv[1:]==[{literal:?}]\nassert os.getcwd()=={vault:?}\nassert 'HOME' not in os.environ\nassert set(os.environ)<= {{'PATH','LANG'}}\nassert event['version']==1 and event['event']=='run_start'\nassert sys.stdin.read()==''\nprint(json.dumps({{'version':1,'action':'continue','context':'python-ok'}}))\n",vault=paths.vault_dir().to_string_lossy());
    let path = script(&paths, &code);
    let hook = definition(vec![
        "python3".into(),
        path.to_string_lossy().into_owned(),
        literal.into(),
    ]);
    let outcome = run(&hook, &paths).await;
    assert_eq!(outcome.status, HookStatus::Completed);
    assert_eq!(outcome.response.unwrap().context(), Some("python-ok"));
    assert!(!paths.vault_dir().join("should-not-exist").exists());
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn node_mjs_observes_literal_args_and_json() {
    let (_temp, paths) = vault();
    let path = paths.config_dir().join("hooks/test.mjs");
    managed_file(&path,"import fs from 'node:fs'; const event=JSON.parse(fs.readFileSync(0,'utf8')); if(event.event!=='run_start'||process.argv[2]!=='$HOME;literal'||process.env.HOME!==undefined) process.exit(17); console.log(JSON.stringify({version:1,action:'continue',context:'node-ok'}));",0o600);
    let outcome = run(
        &definition(vec![
            "node".into(),
            path.to_string_lossy().into_owned(),
            "$HOME;literal".into(),
        ]),
        &paths,
    )
    .await;
    assert_eq!(outcome.status, HookStatus::Completed);
    assert_eq!(outcome.response.unwrap().context(), Some("node-ok"));
}

/// Operator guide contract probe: Go is compiled explicitly, then invoked as a native file.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn compiled_go_file_observes_literal_args_json_and_minimal_environment() {
    let (_temp, paths) = vault();
    let source = paths.config_dir().join("hooks/observe.go");
    let program = paths.config_dir().join("hooks/observe-go");
    managed_file(
        &source,
        r#"package main
import ("encoding/json"; "os")
func main() {
    var event map[string]any
    if json.NewDecoder(os.Stdin).Decode(&event) != nil || event["event"] != "run_start" || len(os.Args) != 2 || os.Args[1] != "$HOME;literal" || os.Getenv("HOME") != "" { os.Exit(17) }
    json.NewEncoder(os.Stdout).Encode(map[string]any{"version":1,"action":"continue","context":"go-ok"})
}"#,
        0o600,
    );
    let build = std::process::Command::new("go")
        .env("GOTOOLCHAIN", "local")
        .env("GO111MODULE", "off")
        .arg("build")
        .arg("-o")
        .arg(&program)
        .arg(&source)
        .status()
        .unwrap();
    assert!(build.success());
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let outcome = run(
        &definition(vec![
            program.to_string_lossy().into_owned(),
            "$HOME;literal".into(),
        ]),
        &paths,
    )
    .await;
    assert_eq!(outcome.status, HookStatus::Completed);
    assert_eq!(outcome.response.unwrap().context(), Some("go-ok"));
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn native_executable_observes_payload_without_persisted_diagnostics() {
    let (_temp, paths) = vault();
    let source = paths.config_dir().join("hooks/native.c");
    let program = paths.config_dir().join("hooks/native");
    managed_file(
        &source,
        r#"#include <stdio.h>
#include <string.h>
#include <stdlib.h>
int main(int argc,char**argv){char input[65537];size_t n=fread(input,1,65536,stdin);input[n]=0;if(argc!=2||strcmp(argv[1],"literal;$(noop)")||getenv("HOME")||!strstr(input,"run_start"))return 17;fprintf(stderr,"private diagnostic secret-token /private/path");puts("{\"version\":1,\"action\":\"continue\",\"context\":\"native-ok\"}");return 0;}
"#,
        0o600,
    );
    let status = std::process::Command::new("cc")
        .arg(&source)
        .arg("-o")
        .arg(&program)
        .status()
        .unwrap();
    assert!(status.success());
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let outcome = run(
        &definition(vec![
            program.to_string_lossy().into_owned(),
            "literal;$(noop)".into(),
        ]),
        &paths,
    )
    .await;
    assert_eq!(outcome.status, HookStatus::Completed);
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!format!("{outcome:?}").contains("secret-token"));
    assert_eq!(outcome.response.unwrap().context(), Some("native-ok"));
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn untrusted_script_forms_and_locations_never_spawn() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (_temp, paths) = vault();
    let marker = paths.vault_dir().join("spawn-marker");
    let code = format!("open({:?},'w').write('spawned')", marker.to_string_lossy());
    let safe = script(&paths, &code);
    let project = paths.wards_dir().join("project");
    std::fs::create_dir(&project).unwrap();
    let project_script = project.join("hook.py");
    managed_file(&project_script, &code, 0o600);
    let link = paths.config_dir().join("hooks/link.py");
    symlink(&project_script, &link).unwrap();
    let writable = paths.config_dir().join("hooks/writable.py");
    managed_file(&writable, &code, 0o666);
    let git = paths.vault_dir().join("repository");
    std::fs::create_dir(&git).unwrap();
    std::fs::create_dir(git.join(".git")).unwrap();
    let git_script = git.join("hook.py");
    managed_file(&git_script, &code, 0o600);
    let mut commands = vec![
        vec![
            "python3".into(),
            project_script.to_string_lossy().into_owned(),
        ],
        vec!["python3".into(), link.to_string_lossy().into_owned()],
        vec!["python3".into(), writable.to_string_lossy().into_owned()],
        vec!["python3".into(), git_script.to_string_lossy().into_owned()],
    ];
    for (runtime, flag) in [
        ("python3", "-c"),
        ("python3", "-m"),
        ("node", "--eval"),
        ("node", "-e"),
        ("node", "--import"),
    ] {
        commands.push(vec![
            runtime.into(),
            flag.into(),
            safe.to_string_lossy().into_owned(),
        ]);
    }
    commands.push(vec![
        "env".into(),
        "python3".into(),
        safe.to_string_lossy().into_owned(),
    ]);
    for command in commands {
        let outcome = run(&definition(command), &paths).await;
        assert_eq!(outcome.failure, Some(HookFailure::UntrustedProgram));
        assert!(!marker.exists());
    }
    std::fs::set_permissions(&safe, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut disabled = definition(vec!["python3".into(), safe.to_string_lossy().into_owned()]);
    disabled.enabled = false;
    assert_eq!(run(&disabled, &paths).await.status, HookStatus::Skipped);
    assert!(!marker.exists());
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn stdin_backpressure_is_inside_deadline() {
    let (_temp, paths) = vault();
    let path = script(&paths, "import time\ntime.sleep(30)\n");
    let mut hook = definition(vec!["python3".into(), path.to_string_lossy().into_owned()]);
    hook.event = HookEvent::UserPrompt;
    hook.timeout_ms = 80;
    let payload = HookEventPayload::new(
        "inv".into(),
        "sess".into(),
        "root".into(),
        HookEventData::UserPrompt {
            prompt: "界".repeat(16384),
        },
    );
    let started = tokio::time::Instant::now();
    let outcome = invoke_hook(&hook, &paths, &[], &payload, &[], &AtomicBool::new(false)).await;
    assert_eq!(outcome.status, HookStatus::Timeout);
    assert!(started.elapsed() < Duration::from_secs(2));
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn concurrent_pipe_bounds_and_nonzero_exit_are_safe_failures() {
    let (_temp, paths) = vault();
    for (code,expected) in [("import sys\nsys.stdin.read()\nsys.stdout.write('x'*32769)\nsys.stdout.flush()\nsys.stderr.write('y'*32769)\nsys.stderr.flush()\n",HookFailure::OutputTooLarge),("import sys\nsys.stdin.read()\nsys.stderr.write('private secret path')\nsys.exit(17)\n",HookFailure::NonZeroExit),("import sys\nsys.stdin.read()\nprint('private malformed secret path')\n",HookFailure::InvalidResponse)] {let path=script(&paths,code);let outcome=run(&definition(vec!["python3".into(),path.to_string_lossy().into_owned()]),&paths).await;assert_eq!(outcome.failure,Some(expected));if expected==HookFailure::NonZeroExit {assert_eq!(outcome.exit_code,Some(17));}assert!(!format!("{outcome:?}").contains("secret"));}
}
#[cfg(target_os = "linux")]
async fn wait_pids(path: &Path) -> Vec<u32> {
    for _ in 0..300 {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(pids) = serde_json::from_str(&text) {
                return pids;
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("fixture did not report pids");
}
#[cfg(target_os = "linux")]
async fn assert_reaped(pids: &[u32]) {
    for _ in 0..300 {
        if pids
            .iter()
            .all(|pid| !Path::new(&format!("/proc/{pid}")).exists())
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    for pid in pids {
        let state = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        assert!(state.is_empty(), "process not reaped: {state}");
    }
}
#[cfg(target_os = "linux")]
fn fork_fixture(paths: &VaultPaths, parent_exits: bool) -> (PathBuf, PathBuf) {
    let pidfile = paths.vault_dir().join("pids.json");
    let code=format!("import os,time,json,sys\nsys.stdin.read()\nchild=os.fork()\nif child==0:\n while True: time.sleep(1)\nopen({pidfile:?},'w').write(json.dumps([os.getpid(),child]))\n{tail}\n",pidfile=pidfile.to_string_lossy(),tail=if parent_exits {"print('{\"version\":1,\"action\":\"continue\"}',flush=True)"}else {"while True: time.sleep(1)"});
    (script(paths, &code), pidfile)
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn stop_and_future_abort_kill_group_and_reap_the_command() {
    for abort in [false, true] {
        let (_temp, paths) = vault();
        let (script, pidfile) = fork_fixture(&paths, false);
        let mut hook = definition(vec![
            "python3".into(),
            script.to_string_lossy().into_owned(),
        ]);
        hook.timeout_ms = 5000;
        let stop = Arc::new(AtomicBool::new(false));
        let task_stop = stop.clone();
        let task_paths = paths.clone();
        let task = tokio::spawn(async move {
            invoke_hook(&hook, &task_paths, &[], &event(), &[], &task_stop).await
        });
        let pids = wait_pids(&pidfile).await;
        if abort {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            stop.store(true, Ordering::Release);
            assert_eq!(task.await.unwrap().status, HookStatus::Cancelled);
        }
        assert_reaped(&pids).await;
    }
}
#[cfg(target_os = "linux")]
#[tokio::test]
async fn timeout_and_success_terminate_inherited_pipe_descendants() {
    for success in [false, true] {
        let (_temp, paths) = vault();
        let (script, pidfile) = fork_fixture(&paths, success);
        let mut hook = definition(vec![
            "python3".into(),
            script.to_string_lossy().into_owned(),
        ]);
        hook.timeout_ms = 200;
        let outcome = run(&hook, &paths).await;
        assert_eq!(
            outcome.status,
            if success {
                HookStatus::Completed
            } else {
                HookStatus::Timeout
            }
        );
        let pids = wait_pids(&pidfile).await;
        assert_reaped(&pids).await;
    }
}
#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn unverified_host_fails_before_spawning() {
    let (_temp, paths) = vault();
    let hook = definition(vec!["unsupported".into()]);
    assert_eq!(
        run(&hook, &paths).await.failure,
        Some(HookFailure::UnsupportedHost)
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn exact_output_limits_drain_both_pipes_and_tool_projection_reaches_the_script() {
    let (_temp, paths) = vault();
    let code = r#"import json,sys,threading
payload=json.load(sys.stdin)
assert payload['data']['arguments']=={'nested':[{'query':'hello [redacted]'}]}
assert payload['data']['arguments_redacted'] is True
response=json.dumps({'version':1,'action':'continue'})
def output():
 sys.stdout.write(response+' '*(32768-len(response)))
 sys.stdout.flush()
def diagnostics():
 sys.stderr.write('x'*32768)
 sys.stderr.flush()
a=threading.Thread(target=output);b=threading.Thread(target=diagnostics)
a.start();b.start();a.join();b.join()
"#;
    let script = script(&paths, code);
    let mut hook = definition(vec![
        "python3".into(),
        script.to_string_lossy().into_owned(),
    ]);
    hook.event = HookEvent::BeforeTool;
    let arguments = serde_json::json!({"nested":[{"query":"hello registered-sentinel","private_key":"secret"}],"headers":[{"name":"authorization","value":"secret"}]});
    let payload = HookEventPayload::new(
        "inv".into(),
        "sess".into(),
        "root".into(),
        HookEventData::BeforeTool {
            tool: "lookup".into(),
            arguments: arguments.clone(),
            arguments_redacted: false,
        },
    );
    let outcome = invoke_hook(
        &hook,
        &paths,
        &[],
        &payload,
        &["registered-sentinel".into()],
        &AtomicBool::new(false),
    )
    .await;
    assert_eq!(outcome.status, HookStatus::Completed);
    assert_eq!(arguments["nested"][0]["query"], "hello registered-sentinel");
}
