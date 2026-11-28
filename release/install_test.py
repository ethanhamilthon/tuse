import errno
import hashlib
import os
import pty
import select
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from http.server import HTTPServer, SimpleHTTPRequestHandler
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INSTALLER = ROOT / "release" / "install.sh"


def uname_asset() -> str:
    os_name = "darwin" if sys.platform == "darwin" else "linux"
    arch = "arm64" if os.uname().machine in ("arm64", "aarch64") else "amd64"
    return f"tuse_{os_name}_{arch}.tar.gz"


def build_release(dest: Path, src: Path | str, bad_sha: bool = False) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    asset = uname_asset()
    if isinstance(src, str):
        bin_path = dest / "tuse"
        bin_path.write_text(src)
        bin_path.chmod(0o755)
    else:
        bin_path = src
    tar_path = dest / asset
    with tarfile.open(tar_path, "w:gz") as tf:
        tf.add(bin_path, arcname="tuse")
    digest = "0" * 64 if bad_sha else hashlib.sha256(tar_path.read_bytes()).hexdigest()
    (dest / "checksums.txt").write_text(f"{digest}  {asset}\n")


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, format, *args):
        pass


def serve(directory: Path):
    srv = HTTPServer(
        ("127.0.0.1", 0), lambda *a, **kw: QuietHandler(*a, directory=str(directory), **kw)
    )
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}"


def install(home: Path, url: str, *args):
    env = {
        "PATH": os.environ["PATH"],
        "HOME": str(home),
        "TUSE_RELEASE_URL": url,
        "TUSE_BIN_DIR": str(home / "bin"),
    }
    return subprocess.run(
        ["/bin/sh", str(INSTALLER), *args],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        env=env,
        timeout=15,
    )


def run_pty(argv, env) -> str:
    pid, fd = pty.fork()
    if pid == 0:
        os.execvpe(argv[0], argv, env)
    output = bytearray()
    deadline = time.monotonic() + 15
    try:
        while time.monotonic() < deadline:
            if not select.select([fd], [], [], max(0, deadline - time.monotonic()))[0]:
                raise AssertionError(bytes(output))
            try:
                chunk = os.read(fd, 4096)
            except OSError as exc:
                if exc.errno != errno.EIO:
                    raise
                break
            if not chunk:
                break
            output.extend(chunk)
        else:
            raise AssertionError(bytes(output))
    finally:
        os.close(fd)
        _, status = os.waitpid(pid, 0)
    assert status == 0, bytes(output)
    return output.decode()


def test_dev_install(root: Path, real_bin: Path) -> None:
    home = root / "dev home"
    config = home / ".config/tusedev/init.lua"
    regular_config = home / ".config/tuse/init.lua"
    dev_db = home / ".tusedev/tuse.db"
    regular_db = home / ".tuse/tuse.db"
    for path in (config, regular_config, dev_db, regular_db):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("keep\n")

    bin_dir = home / "bin"
    bin_dir.mkdir()
    cargo = bin_dir / "cargo"
    cargo.write_text("#!/bin/sh\nexit ${CARGO_TEST_BUILD_STATUS:-0}\n")
    cargo.chmod(0o755)
    target = home / "target"
    (target / "release").mkdir(parents=True)
    (target / "release/tuse").symlink_to(real_bin.resolve())
    env = {
        **os.environ,
        "HOME": str(home),
        "PATH": f"{bin_dir}:{os.environ['PATH']}",
        "CARGO_TARGET_DIR": str(target),
        "TUSE_HOME": str(regular_db.parent),
        "TUSE_GLOBAL_CONFIG": str(regular_config),
        "CLEAN": "1",
    }
    make = ["make", "-C", str(ROOT), "install-dev", f"BIN_DIR={bin_dir}"]
    options = dict(stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=15)

    result = subprocess.run(make, env=env, **options)
    assert result.returncode == 0 and "Resetting" not in result.stdout, result
    assert config.read_text() == dev_db.read_text() == "keep\n"
    result = subprocess.run(
        [*make, "CLEAN=1"], env={**env, "CARGO_TEST_BUILD_STATUS": "1"}, **options
    )
    assert result.returncode != 0, result
    assert config.read_text() == dev_db.read_text() == "keep\n"

    output = run_pty([*make, "CLEAN=1"], env)
    assert "Resetting" in output and "plugin" not in output.lower(), output
    assert config.read_text() == "return {}\n"
    assert not dev_db.exists()
    assert not (home / ".tusedev/plugins").exists()
    assert regular_config.read_text() == regular_db.read_text() == "keep\n"
    assert not (regular_db.parent / "plugins").exists()

    launcher = bin_dir / "tusedev"
    for blocked in ("update", "rollback"):
        r = subprocess.run([str(launcher), blocked], env=env, **options)
        assert r.returncode != 0 and "make install-dev" in r.stderr, r

    result = subprocess.run([*make, "CLEAN=1"], env=env, **options)
    assert result.returncode == 0 and "plugin" not in result.stdout.lower(), result
    assert config.is_file()


def main() -> None:
    real_bin = Path(os.environ["TUSE_BIN"])
    with tempfile.TemporaryDirectory(prefix="tuse-inst-") as tmp:
        root = Path(tmp)
        build_release(root / "ok", real_bin)
        build_release(root / "bad-sha", real_bin, bad_sha=True)
        build_release(root / "bad-version", "#!/bin/sh\nexit 2\n")
        server, url = serve(root)
        try:
            home = root / "fresh"
            result = install(home, f"{url}/ok")
            assert result.returncode == 0 and "Installed tuse " in result.stdout, result
            assert (home / "bin/tuse").is_file() and not (home / ".config").exists()
            assert "plugin" not in result.stdout.lower()
            assert not (home / ".tuse/plugins").exists()

            interactive = root / "interactive"
            pty_env = {
                **os.environ,
                "HOME": str(interactive),
                "TUSE_BIN_DIR": str(interactive / "bin"),
                "TUSE_RELEASE_URL": f"{url}/ok",
            }
            output = run_pty(
                ["/bin/sh", "-c", 'cat "$1" | sh', "sh", str(INSTALLER)],
                pty_env,
            )
            assert "Installed tuse " in output and "plugin" not in output.lower(), output
            assert not (interactive / ".tuse/plugins").exists()

            config = home / ".config/tuse/init.lua"
            config.parent.mkdir(parents=True)
            config.write_text("return { settings = { compact_at = 1 } }\n")
            result = install(home, f"{url}/ok")
            assert result.returncode == 0, result
            assert config.read_text() == "return { settings = { compact_at = 1 } }\n"

            home = root / "bad-checksum"
            result = install(home, f"{url}/bad-sha")
            assert result.returncode != 0 and "checksum mismatch" in result.stderr, result
            assert not home.exists()

            home = root / "bad-binary"
            result = install(home, f"{url}/bad-version")
            assert result.returncode == 2 and "Installed" not in result.stdout, result
            assert not (home / ".config").exists()

            home = root / "removed-option"
            result = install(home, f"{url}/ok", "--no-setup")
            assert result.returncode != 0 and "unknown option" in result.stderr, result
            assert not home.exists()

            test_dev_install(root, real_bin)
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    main()
