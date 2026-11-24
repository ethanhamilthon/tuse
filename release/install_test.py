import hashlib
import os
import subprocess
import sys
import tarfile
import tempfile
import threading
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

            config = home / ".config/tuse/init.lua"
            config.parent.mkdir(parents=True)
            config.write_text("return { settings = { retries = 1 } }\n")
            result = install(home, f"{url}/ok")
            assert result.returncode == 0, result
            assert config.read_text() == "return { settings = { retries = 1 } }\n"

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
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    main()
