"""Every use case in the docs runs here, so the docs never show code that does not work."""

import os
import sys
import tempfile
import unittest
from pathlib import Path

import test_runtime as runtime

sys.path.insert(0, str(runtime.ROOT / "python"))
sys.path.insert(0, str(runtime.ROOT / "examples"))
os.environ["PYNOX_BIN"] = str(runtime.BINARY)

from use_cases import api_agent, coding_agent, data_analyst, support_agent  # noqa: E402


class UseCaseTests(unittest.TestCase):
    def test_data_analyst_keeps_state_writes_the_report_and_cannot_read_secrets(self):
        with tempfile.TemporaryDirectory() as temporary:
            transcript = data_analyst.run(Path(temporary))
            answers = [answer for _, answer in transcript]
            self.assertEqual(answers[0], "400 rows; columns: region, quarter, revenue\n")
            self.assertEqual(answers[1], "south grew 31.0%\n")
            self.assertEqual(answers[2], "report written\n")
            self.assertTrue(
                answers[3].startswith("PermissionError") or "FileNotFoundError" in answers[3],
                answers[3],
            )
            report = (Path(temporary) / "out" / "report.md").read_text()
            self.assertIn("- south: +31.0%", report)

    def test_support_agent_chains_host_functions_under_host_rules(self):
        with tempfile.TemporaryDirectory() as temporary:
            outcome = support_agent.run(Path(temporary))
        self.assertEqual(
            outcome["answer"],
            "A-101: PermissionError: refunds above 200 need approval; ticket opened\n"
            "checked 2 orders, 1 damaged\n",
        )
        self.assertEqual(outcome["refunds"], [], "the host rule held")
        # Landlock denies the open; sqlite reports it as its own error type.
        self.assertIn("unable to open database file", outcome["bypass"])
        self.assertIn("find_orders(customer: str) -> list[dict]", outcome["doc"])

    def test_coding_agent_tests_run_but_cannot_delete_outside_the_project(self):
        with tempfile.TemporaryDirectory() as temporary:
            outcome = coding_agent.run(Path(temporary), str(runtime.BINARY))
        self.assertEqual(outcome["exit"], 1, outcome["stderr"])
        self.assertIn("PermissionError", outcome["stderr"])
        self.assertIn("Ran 2 tests", outcome["stderr"])
        self.assertTrue(outcome["notes_survived"])

    def test_api_agent_needs_the_network_grant_and_gets_the_key_from_the_host(self):
        outcome = api_agent.run()
        self.assertIn("Operation not permitted", outcome["offline"])
        self.assertEqual(outcome["online"], "{'city': 'Lisbon', 'temp_c': 21}\n")
