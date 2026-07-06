import os
import stat

import pytest

from reps_for_claude.config import Config
from reps_for_claude import shim


def make_fake_claude(directory, name="claude"):
    directory.mkdir(parents=True, exist_ok=True)
    binary = directory / name
    binary.write_text("#!/bin/sh\necho real claude\n")
    binary.chmod(binary.stat().st_mode | stat.S_IXUSR)
    return binary


class TestFindRealClaude:
    def test_config_override_wins(self):
        cfg = Config(real_claude="/opt/claude")
        assert shim.find_real_claude(cfg) == "/opt/claude"

    def test_finds_on_path(self, tmp_path, monkeypatch):
        real = make_fake_claude(tmp_path / "realbin")
        monkeypatch.setenv("PATH", str(tmp_path / "realbin"))
        assert shim.find_real_claude(Config()) == str(real)

    def test_skips_shim_dir(self, tmp_path, monkeypatch):
        shim_dir = tmp_path / "shimbin"
        make_fake_claude(shim_dir)
        real = make_fake_claude(tmp_path / "realbin")
        monkeypatch.setenv(
            "PATH", os.pathsep.join([str(shim_dir), str(tmp_path / "realbin")])
        )
        assert shim.find_real_claude(Config(), shim_dir=shim_dir) == str(real)

    def test_skips_installed_shims_by_marker(self, tmp_path, monkeypatch):
        other_shim_dir = tmp_path / "othershim"
        other_shim_dir.mkdir()
        fake = other_shim_dir / "claude"
        fake.write_text(f"#!/bin/sh\n{shim.SHIM_MARKER}\nexec whatever\n")
        fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
        real = make_fake_claude(tmp_path / "realbin")
        monkeypatch.setenv(
            "PATH", os.pathsep.join([str(other_shim_dir), str(tmp_path / "realbin")])
        )
        assert shim.find_real_claude(Config()) == str(real)

    def test_none_when_absent(self, monkeypatch, tmp_path):
        empty = tmp_path / "empty"
        empty.mkdir()
        monkeypatch.setenv("PATH", str(empty))
        assert shim.find_real_claude(Config()) is None


class TestInstallUninstall:
    def test_install_writes_executable_shim(self, tmp_path, monkeypatch):
        real = make_fake_claude(tmp_path / "realbin")
        monkeypatch.setenv("PATH", str(tmp_path / "realbin"))
        bin_dir = tmp_path / "shimbin"
        path = shim.install(Config(), bin_dir)
        assert path == bin_dir / "claude"
        content = path.read_text()
        assert shim.SHIM_MARKER in content
        assert f"reps guard -- {real}" in content
        assert os.access(path, os.X_OK)

    def test_install_without_real_claude_fails(self, tmp_path, monkeypatch):
        empty = tmp_path / "empty"
        empty.mkdir()
        monkeypatch.setenv("PATH", str(empty))
        with pytest.raises(FileNotFoundError):
            shim.install(Config(), tmp_path / "shimbin")

    def test_uninstall_removes_shim(self, tmp_path, monkeypatch):
        make_fake_claude(tmp_path / "realbin")
        monkeypatch.setenv("PATH", str(tmp_path / "realbin"))
        bin_dir = tmp_path / "shimbin"
        shim.install(Config(), bin_dir)
        assert shim.uninstall(bin_dir) is True
        assert not (bin_dir / "claude").exists()

    def test_uninstall_refuses_non_shim(self, tmp_path):
        bin_dir = tmp_path / "bin"
        make_fake_claude(bin_dir)  # a real binary, not our shim
        with pytest.raises(RuntimeError, match="refusing"):
            shim.uninstall(bin_dir)

    def test_uninstall_missing_is_false(self, tmp_path):
        assert shim.uninstall(tmp_path / "nothing") is False
