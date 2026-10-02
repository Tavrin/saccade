#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::Path;
use std::process::Command;

use image::{Rgb, RgbImage};

fn save(dir: &Path, name: &str, v: u8) {
    std::fs::create_dir_all(dir).expect("mkdir");
    RgbImage::from_pixel(16, 16, Rgb([v, v, v]))
        .save(dir.join(name))
        .expect("save");
}

fn saccade(args: &[&Path]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .expect("spawn")
}

#[test]
fn exit_codes_and_approve() {
    let tmp = tempfile::tempdir().expect("tmp");
    let (base, cap, out) = (
        tmp.path().join("base"),
        tmp.path().join("cap"),
        tmp.path().join("out"),
    );
    save(&base, "a.png", 100);
    save(&cap, "a.png", 100);
    let compare = |out: &Path| {
        let o = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .args(["compare"])
            .args([&base, &cap])
            .arg("--out")
            .arg(out)
            .current_dir(tmp.path())
            .output()
            .expect("spawn");
        o.status.code()
    };
    assert_eq!(compare(&out), Some(0));

    // A regression exits 1 and the table names the failing image.
    save(&cap, "a.png", 200);
    assert_eq!(compare(&out), Some(1));

    // Approving it makes the next run clean again.
    let report = out.join("saccade-report.v1.json");
    let approve = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["approve"])
        .args([&cap, &base])
        .arg("--all-failing")
        .arg(&report)
        .output()
        .expect("spawn");
    assert_eq!(approve.status.code(), Some(0));
    assert_eq!(compare(&out), Some(0));

    // Usage and IO errors exit 2.
    assert_eq!(saccade(&[Path::new("compare")]).status.code(), Some(2));
    let missing = tmp.path().join("nope");
    assert_eq!(
        saccade(&[Path::new("compare"), &missing, &cap])
            .status
            .code(),
        Some(2)
    );
}

#[cfg(feature = "workbench")]
#[test]
fn serve_skips_unresolvable_symlink_targets_but_requires_positional_roots() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::process::{Child, Stdio};
    use std::time::{Duration, Instant};

    struct Server(Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let valid = tmp.path().join("mounted");
    let missing = tmp.path().join("unmounted");
    let file = tmp.path().join("file");
    let cache = tmp.path().join("cache");
    let decisions = tmp.path().join("decisions");
    let stderr = tmp.path().join("stderr.txt");
    save(&root, "a.png", 100);
    save(&valid, "a.png", 200);
    std::fs::write(&file, "not a directory").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&valid, root.join("linked")).unwrap();
        std::os::unix::fs::symlink(&missing, root.join("later")).unwrap();
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_saccade"));
    command
        .current_dir(tmp.path())
        .arg("serve")
        .arg(&root)
        .arg("--symlink-target")
        .arg(&missing)
        .arg("--symlink-target")
        .arg(&file)
        .arg("--symlink-target")
        .arg(&valid)
        .args(["--port", "0", "--cache-dir"])
        .arg(&cache)
        .arg("--decisions-dir")
        .arg(&decisions)
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&stderr).unwrap());
    #[cfg(unix)]
    {
        let cyclic = tmp.path().join("cyclic");
        std::os::unix::fs::symlink(&cyclic, &cyclic).unwrap();
        command.arg("--symlink-target").arg(cyclic);
    }
    let mut server = Server(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !cache.join("serve.json").is_file() {
        assert!(
            server.0.try_wait().unwrap().is_none() && Instant::now() < deadline,
            "server did not start: {}",
            std::fs::read_to_string(&stderr).unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let discovery: serde_json::Value =
        serde_json::from_slice(&std::fs::read(cache.join("serve.json")).unwrap()).unwrap();
    let get = |path: &str| {
        let port = discovery["port"].as_u64().unwrap() as u16;
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    };
    assert!(get("/api/roots").starts_with("HTTP/1.1 200"));
    #[cfg(unix)]
    {
        // A skipped target does not become authorized if it appears later.
        save(&missing, "a.png", 150);
        assert!(get("/api/images?path=linked").starts_with("HTTP/1.1 200"));
        assert!(get("/api/images?path=later").starts_with("HTTP/1.1 403"));
    }
    let warnings = std::fs::read_to_string(&stderr).unwrap();
    for target in [&missing, &file] {
        assert!(
            warnings.contains(&format!(
                "warning: skipping --symlink-target {}:",
                target.display()
            )),
            "{warnings}"
        );
    }
    #[cfg(unix)]
    assert!(warnings.contains(&format!(
        "warning: skipping --symlink-target {}:",
        tmp.path().join("cyclic").display()
    )));
    drop(server);

    let absent = tmp.path().join("missing-root");
    for roots in [vec![&absent], vec![&root, &absent]] {
        let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
            .current_dir(tmp.path())
            .arg("serve")
            .args(roots)
            .args(["--port", "0"])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("resolving archive root"));
    }
}
