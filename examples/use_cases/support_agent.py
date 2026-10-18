"""A support agent that calls your backend through host functions.

The model's code chains lookups and a refund in one run. The functions run in
your process, where the database and the business rules live; the sandboxed
code can call them but cannot open the database itself.
"""

import json
import sqlite3
import tempfile
from pathlib import Path

import pynox


def make_backend(root: Path):
    database = root / "orders.db"
    with sqlite3.connect(database) as db:
        db.execute("create table orders (id text, customer text, total real, status text)")
        db.executemany(
            "insert into orders values (?, ?, ?, ?)",
            [
                ("A-100", "ana@example.com", 42.0, "delivered"),
                ("A-101", "ana@example.com", 310.0, "damaged"),
                ("B-200", "bo@example.com", 18.5, "delivered"),
            ],
        )
    refunds = []

    def find_orders(customer: str) -> list[dict]:
        """List a customer's orders with id, total and status."""
        with sqlite3.connect(database) as db:
            rows = db.execute(
                "select id, total, status from orders where customer = ?", (customer,)
            ).fetchall()
        return [{"id": i, "total": t, "status": s} for i, t, s in rows]

    def issue_refund(order_id: str, amount: float) -> str:
        """Refund an order. Amounts above 200 need a human."""
        if amount > 200:
            raise PermissionError("refunds above 200 need approval; ticket opened")
        refunds.append((order_id, amount))
        return f"refunded {amount:.2f} on {order_id}"

    return database, refunds, {"find_orders": find_orders, "issue_refund": issue_refund}


# What a model writes for "Ana says an order arrived damaged. Sort it out."
MODEL_CODE = """
from pynox_host import find_orders, issue_refund, HostError

orders = find_orders("ana@example.com")
damaged = [o for o in orders if o["status"] == "damaged"]
for order in damaged:
    try:
        print(issue_refund(order["id"], order["total"]))
    except HostError as error:
        print(f"{order['id']}: {error}")
print(f"checked {len(orders)} orders, {len(damaged)} damaged")
"""


def run(root: Path) -> dict:
    database, refunds, functions = make_backend(root)
    run_python = pynox.tool(functions=functions)
    answer = run_python(MODEL_CODE)
    # The model then tries to skip the rules and edit the database directly.
    bypass = run_python(
        f"import sqlite3\nsqlite3.connect({str(database)!r}).execute(\"update orders set status='refunded'\")"
    )
    return {"answer": answer, "bypass": bypass, "refunds": refunds, "doc": run_python.__doc__}


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as temporary:
        print(json.dumps(run(Path(temporary)), indent=2))
