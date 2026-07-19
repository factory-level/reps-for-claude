import json

from reps_for_claude.report import finish, render_markdown, render_weekly_markdown, review_counts


def test_render_weekly_markdown():
    md = render_weekly_markdown(
        week="2026-W29",
        targets={"squat": 60, "bench": 40},
        reps={"squat": 45, "bench": 40},
        volume_lbs={"squat": 2025.0, "bench": 3200.0},
        jumprope_seconds=180.0,
    )
    assert "# Weekly Volume — 2026-W29" in md
    assert "| squat | 60 | 45 | 2025 |" in md
    assert "| bench | 40 | 40 | 3200 |" in md
    assert "Jump rope: 3.0 min" in md


class TestRenderMarkdown:
    def test_structure(self):
        md = render_markdown(
            "2026-07-06",
            plan={"pushup": 30, "squat": 40},
            reps={"pushup": 30, "squat": 40, "curl": 12},
            spent_seconds=1800.0,
            balance_seconds=600.0,
            complete=True,
        )
        assert "# Workout Form — 2026-07-06" in md
        assert "| pushup | 30 | 30 |" in md
        assert "| curl | — | 12 |" in md  # off-plan exercise shows no target
        assert "Workout complete: yes" in md
        assert "Claude time spent: 30.0 min" in md
        assert "Credit remaining: 10.0 min" in md


class TestReviewCounts:
    def test_blank_keeps_counts(self):
        counts, edited = review_counts(
            {"pushup": 10}, {"pushup": 8}, input_fn=lambda p: "", print_fn=lambda m: None
        )
        assert counts == {"pushup": 8}
        assert not edited

    def test_edit_overrides(self):
        counts, edited = review_counts(
            {"pushup": 10}, {"pushup": 8}, input_fn=lambda p: "12", print_fn=lambda m: None
        )
        assert counts == {"pushup": 12}
        assert edited

    def test_invalid_input_keeps_and_warns(self):
        warnings = []
        counts, edited = review_counts(
            {"pushup": 10},
            {"pushup": 8},
            input_fn=lambda p: "many",
            print_fn=warnings.append,
        )
        assert counts == {"pushup": 8}
        assert not edited
        assert any("invalid" in w for w in warnings)

    def test_covers_plan_and_offplan(self):
        counts, _ = review_counts(
            {"pushup": 10}, {"curl": 5}, input_fn=lambda p: "", print_fn=lambda m: None
        )
        assert counts == {"pushup": 0, "curl": 5}


class TestFinish:
    def test_writes_md_and_json(self, cfg, ledger, tmp_path):
        ledger.add_reps("pushup", 10)
        ledger.add_reps("squat", 5)
        ledger.set_balance(300.0)
        logs = tmp_path / "logs"
        md_path, json_path = finish(
            ledger, cfg, logs, input_fn=lambda p: "", print_fn=lambda m: None
        )
        assert md_path.name == "2026-07-06.md"
        data = json.loads(json_path.read_text())
        assert data["reps"] == {"pushup": 10, "squat": 5}
        assert data["workout_complete"] is True
        assert data["edited"] is False
        assert ledger.state.report_done

    def test_edits_are_persisted(self, cfg, ledger, tmp_path):
        ledger.add_reps("pushup", 8)
        answers = iter(["12", ""])  # pushup -> 12, squat blank (0)
        _, json_path = finish(
            ledger,
            cfg,
            tmp_path / "logs",
            input_fn=lambda p: next(answers),
            print_fn=lambda m: None,
        )
        data = json.loads(json_path.read_text())
        assert data["reps"]["pushup"] == 12
        assert data["edited"] is True
        assert ledger.state.reps["pushup"] == 12
