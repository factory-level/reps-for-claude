from reps_for_claude.duesstate import FileDuesState


def test_defaults_to_not_owed(tmp_path):
    assert FileDuesState(tmp_path).owed() is False


def test_roundtrip(tmp_path):
    d = FileDuesState(tmp_path)
    d.set_owed(True)
    assert FileDuesState(tmp_path).owed() is True
    d.set_owed(False)
    assert FileDuesState(tmp_path).owed() is False


def test_corrupt_reads_not_owed(tmp_path):
    (tmp_path / "dues.json").write_text("garbage")
    assert FileDuesState(tmp_path).owed() is False


def test_unreadable_reads_not_owed(tmp_path):
    import os
    d = FileDuesState(tmp_path)
    d.set_owed(True)
    path = tmp_path / "dues.json"
    os.chmod(path, 0o000)
    try:
        assert FileDuesState(tmp_path).owed() is False
    finally:
        os.chmod(path, 0o600)  # restore so tmp_path cleanup works
