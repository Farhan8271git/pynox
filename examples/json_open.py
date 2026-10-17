"""Unchanged application for stock CPython and the experimental runtime."""

import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    orders = json.load(source)

for order in orders:
    order["total"] = order["price"] * order["quantity"]

with open(sys.argv[2], "w", encoding="utf-8") as destination:
    json.dump(orders, destination)

print(sum(order["total"] for order in orders))
