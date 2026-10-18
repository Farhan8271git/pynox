"""A data analyst agent: notebook-style analysis over local files.

The model explores a CSV across several turns, keeps variables between them,
writes a report, and tries to read something it should not. The host allows
reading ./data and writing ./out, for 20 seconds per step.
"""

import csv
import random
import tempfile
from pathlib import Path

import pynox

# What a model writes, turn by turn, for "Which region grew the most? Write a report."
TURNS = [
    "import csv\n"
    "with open(DATA) as f:\n"
    "    rows = list(csv.DictReader(f))\n"
    "print(len(rows), 'rows;', 'columns:', ', '.join(rows[0]))",
    "from collections import defaultdict\n"
    "totals = defaultdict(lambda: [0.0, 0.0])\n"
    "for r in rows:\n"
    "    totals[r['region']][r['quarter'] == 'Q2'] += float(r['revenue'])\n"
    "growth = {k: (q2 - q1) / q1 for k, (q1, q2) in totals.items()}\n"
    "best = max(growth, key=growth.get)\n"
    "print(f'{best} grew {growth[best]:.1%}')",
    "with open(OUT, 'w') as f:\n"
    "    f.write('# Revenue growth Q1 to Q2\\n\\n')\n"
    "    for k, g in sorted(growth.items(), key=lambda kv: -kv[1]):\n"
    "        f.write(f'- {k}: {g:+.1%}\\n')\n"
    "print('report written')",
    # The model gets curious about the project's secrets "to check the data source".
    "print(open(ENV).read())",
]


def make_data(root: Path) -> Path:
    rng = random.Random(7)
    data = root / "data"
    data.mkdir()
    with open(data / "sales.csv", "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["region", "quarter", "revenue"])
        for region, trend in [("north", 1.05), ("south", 1.31), ("east", 0.92), ("west", 1.12)]:
            for _ in range(50):
                base = rng.uniform(800, 1200)
                writer.writerow([region, "Q1", f"{base:.2f}"])
                writer.writerow([region, "Q2", f"{base * trend:.2f}"])
    (root / "out").mkdir()
    (root / ".env").write_text("WAREHOUSE_PASSWORD=hunter2\n")
    return data / "sales.csv"


def run(root: Path) -> list[tuple[str, str]]:
    source = make_data(root)
    report = root / "out" / "report.md"
    notebook = pynox.tool(
        session=True, allow_read=[root / "data"], allow_write=[root / "out"], timeout="20s"
    )
    notebook(f"DATA = {str(source)!r}\nOUT = {str(report)!r}\nENV = {str(root / '.env')!r}")
    return [(code, notebook(code)) for code in TURNS]


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as temporary:
        for code, answer in run(Path(temporary)):
            print(f">>> model code\n{code}\n<<< tool result\n{answer}\n")
        print((Path(temporary) / "out" / "report.md").read_text())
