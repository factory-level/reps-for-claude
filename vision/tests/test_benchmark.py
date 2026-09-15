"""The benchmark must reject counts occurring at the wrong time."""
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location('benchmark_movements', Path(__file__).parents[1] / 'scripts/benchmark_movements.py')
benchmark = importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)


def test_equal_counts_with_wrong_events_fail():
    assert benchmark.match_completions([1000, 7000], [7000, 10000]) == {
        'truePositives': 1, 'falsePositives': 1, 'falseNegatives': 1}


def test_matching_is_one_to_one_and_includes_tolerance_boundary():
    assert benchmark.match_completions([1000, 1100, 2500], [1500, 3000]) == {
        'truePositives': 2, 'falsePositives': 1, 'falseNegatives': 0}


def test_negative_example_rejects_any_detection():
    assert benchmark.match_completions([1000], [])['falsePositives'] == 1
    assert benchmark.match_completions([], []) == {
        'truePositives': 0, 'falsePositives': 0, 'falseNegatives': 0}
