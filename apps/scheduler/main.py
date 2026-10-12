"""Run the local scheduler against a versioned JSON snapshot."""

import argparse
import json

from analyzer import analyze
from preference_compiler import compile_wishes
from solver import generate


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("snapshot")
    parser.add_argument("--operation", choices=("generate", "analyze", "compile-preferences"), default="generate")
    parser.add_argument("--seconds", type=int, help="optional CP-SAT time limit; omit for no fixed deadline")
    args = parser.parse_args()
    with open(args.snapshot, encoding="utf-8") as source:
        data = json.load(source)
    if data.get("schema_version") != 1:
        parser.error("unsupported snapshot schema version")
    if args.seconds is not None and args.seconds < 1:
        parser.error("seconds must be a positive integer")
    if args.operation == "analyze":
        result = {"findings": analyze(data, data.get("sandbox_proposal", []))}
    elif args.operation == "compile-preferences":
        result = compile_wishes(data)
    else:
        result = generate(data, seconds=args.seconds)
    print(json.dumps(result, ensure_ascii=False, separators=(",", ":")))


if __name__ == "__main__":
    main()
