import fcntl
import hashlib
import os
import pty
import select
import shutil
import subprocess
import sys
import tarfile
import tempfile
import termios
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


def run_pty(cmd, env, steps=(), piped=False, sigint_on=None, hook=None):
    master, slave = pty.openpty()
    pipe_r = None
    if piped:
        pipe_r, pipe_w = os.pipe()
        data = INSTALLER.read_bytes()

        def feed():
            with os.fdopen(pipe_w, "wb") as f:
                f.write(data)

        threading.Thread(target=feed, daemon=True).start()

    def preexec():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    proc = subprocess.Popen(
        cmd,
        stdin=pipe_r if piped else slave,
        stdout=slave,
        stderr=slave,
        env=env,
        preexec_fn=preexec,
        close_fds=True,
    )
    os.close(slave)
    if pipe_r is not None:
        os.close(pipe_r)

    buf = b""
    idx = 0
    hooked = False
    signaled = False
    deadline = time.time() + 15
    while time.time() < deadline:
        ready, _, _ = select.select([master], [], [], 0.05)
        if ready:
            try:
                chunk = os.read(master, 4096)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk
            text = buf.decode("utf-8", errors="replace")
            if hook and not hooked and hook[0] in text:
                hook[1]()
                hooked = True
            if sigint_on and not signaled and sigint_on in text:
                os.write(master, b"\x03")
                signaled = True
            while idx < len(steps):
                expect, reply = steps[idx]
                if expect in text:
                    os.write(master, b"\x04" if reply is None else reply.encode())
                    idx += 1
                else:
                    break
        if proc.poll() is not None:
            while select.select([master], [], [], 0.05)[0]:
                try:
                    chunk = os.read(master, 4096)
                except OSError:
                    break
                if not chunk:
                    break
                buf += chunk
            break
    os.close(master)
    return proc.wait(timeout=5), buf.decode("utf-8", errors="replace")


def run_no_tty(cmd, env):
    p = subprocess.run(
        cmd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        preexec_fn=os.setsid,
        timeout=15,
    )
    return p.returncode, p.stdout.decode() + p.stderr.decode()


MOCK_BUNDLED = """#!/bin/sh
if [ "${1:-}" = "version" ]; then echo "tuse 0.9.3"; exit 0; fi
if [ "${1:-}" = "models" ] && [ "${2:-}" = "--config" ]; then
  grep -q 'tuse/default' "$3" && exit 0
  exit 1
fi
exit 0
"""

MOCK_BAD_VERSION = """#!/bin/sh
if [ "${1:-}" = "version" ]; then echo "broken" >&2; exit 2; fi
exit 0
"""


def main() -> None:
    real_bin = Path(os.environ["TUSE_BIN"])
    with tempfile.TemporaryDirectory(prefix="tuse-inst-") as td_raw:
        td = Path(td_raw)
        build_release(td / "rel_new", MOCK_BUNDLED)
        build_release(td / "rel_real", real_bin)
        build_release(td / "rel_bad_ver", MOCK_BAD_VERSION)
        build_release(td / "rel_bad_sha", MOCK_BUNDLED, bad_sha=True)

        srvs = []
        srv, url_new = serve(td / "rel_new")
        srvs.append(srv)
        srv, url_real = serve(td / "rel_real")
        srvs.append(srv)
        srv, url_bad_ver = serve(td / "rel_bad_ver")
        srvs.append(srv)
        srv, url_bad_sha = serve(td / "rel_bad_sha")
        srvs.append(srv)

        def mk_env(name: str, url: str = url_new, extra=None):
            home = td / f"h_{name}"
            home.mkdir(parents=True, exist_ok=True)
            env = {
                "PATH": os.environ["PATH"],
                "HOME": str(home),
                "TUSE_RELEASE_URL": url,
                "TUSE_BIN_DIR": str(home / ".local/bin"),
            }
            if extra:
                env.update(extra)
            return home, env

        try:
            home, env = mk_env("both")
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env, [("[Y/n]", "y\n"), ("[y/N]", "yes\n")])
            assert rc == 0 and "/subs" in out, (rc, out)
            assert '{ "tuse/default", opts = { theme = true, subs = true } }' in (home / ".config/tuse/init.lua").read_text()

            home, env = mk_env("subs_only")
            rc, _ = run_pty(
                ["/bin/sh", str(INSTALLER)],
                env,
                [("[Y/n]", "bad\n"), ("Please answer y or n.", "n\n"), ("[y/N]", "y\n")],
            )
            assert rc == 0
            assert '{ "tuse/default", opts = { theme = false, subs = true } }' in (home / ".config/tuse/init.lua").read_text()

            home, env = mk_env("none")
            rc, _ = run_pty(["/bin/sh", str(INSTALLER)], env, [("[Y/n]", "n\n"), ("[y/N]", "n\n")])
            assert rc == 0 and (home / ".config/tuse/init.lua").read_text().strip() == "return {}"

            home, env = mk_env("piped")
            rc, out = run_pty(["/bin/sh"], env, [("[Y/n]", "\n"), ("[y/N]", "\n")], piped=True)
            assert rc == 0, (rc, out)
            assert '{ "tuse/default", opts = { theme = true, subs = false } }' in (home / ".config/tuse/init.lua").read_text()

            home, env = mk_env("existing_symlink")
            (home / ".config").mkdir(parents=True)
            os.symlink("/nonexistent/cfg.lua", home / ".config/tuse.lua")
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env)
            assert rc == 0 and "Keeping existing config:" in out
            assert not (home / ".config/tuse/init.lua").exists()

            real_link = shutil.which("link")
            assert real_link, "link command not found"
            for kind in ("file", "dir", "sym_file", "sym_dir", "sym_dangling"):
                home, env = mk_env(f"race_{kind}")
                race_target = td / f"race_tgt_{kind}"
                shim_dir = td / f"shim_{kind}"
                shim_dir.mkdir()
                called = td / f"link_called_{kind}"
                if kind == "file":
                    setup_race = 'printf "%s\\n" "-- raced" "return {}" > "$2"'
                elif kind == "dir":
                    setup_race = 'mkdir -p "$2"'
                elif kind == "sym_file":
                    race_target.write_text("-- symfile\nreturn {}\n")
                    setup_race = f'ln -s "{race_target}" "$2"'
                elif kind == "sym_dir":
                    race_target.mkdir()
                    setup_race = f'ln -s "{race_target}" "$2"'
                else:
                    setup_race = f'ln -s "{td / "missing_target.lua"}" "$2"'
                shim = shim_dir / "link"
                shim.write_text(
                    f'#!/bin/sh\nset -eu\n: > "{called}"\n{setup_race}\nexec "{real_link}" "$@"\n'
                )
                shim.chmod(0o755)
                env["PATH"] = f"{shim_dir}:{env['PATH']}"

                rc, out = run_pty(
                    ["/bin/sh", str(INSTALLER)],
                    env,
                    [("[Y/n]", "y\n"), ("[y/N]", "n\n")],
                )
                assert called.exists(), kind
                assert rc == 0 and "Keeping existing config:" in out, (kind, rc, out)
                p = home / ".config/tuse/init.lua"
                assert list((home / ".config/tuse").iterdir()) == [p], kind
                if kind == "file":
                    assert p.read_text() == "-- raced\nreturn {}\n"
                elif kind == "dir":
                    assert p.is_dir() and list(p.iterdir()) == []
                elif kind == "sym_file":
                    assert os.path.islink(p) and race_target.read_text() == "-- symfile\nreturn {}\n"
                elif kind == "sym_dir":
                    assert os.path.islink(p) and list(race_target.iterdir()) == []
                elif kind == "sym_dangling":
                    assert os.path.islink(p) and not p.exists()

            fake_bin = td / "fake_no_link"
            fake_bin.mkdir()
            (fake_bin / "link").write_text("#!/bin/sh\necho 'operation not supported' >&2\nexit 1\n")
            (fake_bin / "link").chmod(0o755)
            home, env = mk_env("fail_closed", extra={"PATH": f"{fake_bin}:{os.environ['PATH']}"})
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env, [("[Y/n]", "y\n"), ("[y/N]", "n\n")])
            assert rc != 0 and "failed to write" in out, (rc, out)
            assert not (home / ".config/tuse/init.lua").exists()
            assert list((home / ".config/tuse").iterdir()) == []

            home, env = mk_env("eof")
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env, [("[Y/n]", None)])
            assert rc == 0 and "Skipped first-run setup." in out, (rc, out)
            assert not (home / ".config/tuse/init.lua").exists()

            home, env = mk_env("sigint")
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env, sigint_on="[Y/n]")
            assert rc == 130, (rc, out)
            assert not (home / ".config/tuse/init.lua").exists()

            for name, cmd, extra in [
                ("env_nosetup", ["/bin/sh", str(INSTALLER)], {"TUSE_NO_SETUP": "1"}),
                ("flag_nosetup", ["/bin/sh", str(INSTALLER), "--no-setup"], {}),
                ("env_ci", ["/bin/sh", str(INSTALLER)], {"CI": "true"}),
            ]:
                home, env = mk_env(name, extra=extra)
                rc, out = run_pty(cmd, env)
                assert rc == 0 and not (home / ".config/tuse/init.lua").exists(), (name, rc, out)

            home, env = mk_env("no_tty")
            rc, out = run_no_tty(["/bin/sh", str(INSTALLER)], env)
            assert rc == 0 and not (home / ".config/tuse/init.lua").exists(), (rc, out)

            home, env = mk_env("bad_ver", url=url_bad_ver)
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env)
            assert rc != 0 and not (home / ".config/tuse").exists(), (rc, out)

            home, env = mk_env("bad_sha", url=url_bad_sha)
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env)
            assert rc != 0 and "checksum mismatch" in out and not (home / ".config/tuse").exists()

            home, env = mk_env("old_with_checkout", url=url_real)
            sentinel = home / "pwned.txt"
            checkout = home / ".tuse/plugins/tuse/default"
            checkout.mkdir(parents=True)
            (checkout / "init.lua").write_text(
                f'local f = io.open("{sentinel}", "w"); if f then f:write("ran"); f:close() end; return {{}}\n'
            )
            rc, out = run_pty(["/bin/sh", str(INSTALLER)], env, [("[Y/n]", "y\n"), ("[y/N]", "n\n")])
            assert rc == 0, (rc, out)
            assert not sentinel.exists(), out
            cfg = (home / ".config/tuse/init.lua").read_text()
            assert '"tuse/default"' in cfg and "theme = true" in cfg and "subs = false" in cfg, cfg
            probe = subprocess.run(
                [str(home / ".local/bin/tuse"), "models", "--config", str(home / ".config/tuse/init.lua")],
                env=env, capture_output=True, text=True,
            )
            assert probe.returncode == 0 and not sentinel.exists(), probe.stderr
        finally:
            for s in srvs:
                s.shutdown()


if __name__ == "__main__":
    main()
