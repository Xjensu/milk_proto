"""Shared generated-binding fixtures for mobile surface and artifact tests."""

from __future__ import annotations

from collections.abc import Collection, Iterable


def kotlin_surface(methods: Iterable[str], throwing_methods: Collection[str]) -> str:
    body = "\n".join(
        (
            "    @Throws(FfiException::class)\n"
            if method in throwing_methods
            else ""
        )
        + f"    override fun `{method}`() {{}}"
        for method in methods
    )
    return f"interface MmpNodeInterface {{\n{body}\n}}\n"


def swift_surface(methods: Iterable[str], throwing_methods: Collection[str]) -> str:
    body = "\n".join(
        f"    func {method}(){' throws' if method in throwing_methods else ''}"
        for method in methods
    )
    return f"protocol MmpNodeProtocol {{\n{body}\n}}\n"
