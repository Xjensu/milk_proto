"""Shared generated-binding fixtures for mobile surface and artifact tests."""

from __future__ import annotations

from collections.abc import Collection, Iterable


def _type_declarations(
    declarations: Iterable[tuple[str, str]], language: str
) -> str:
    """Render the minimal public declarations used by source-level fixtures."""

    lines: list[str] = []
    for name, kind in declarations:
        if language == "kotlin":
            name = "FfiException" if name == "FfiError" else name
            keyword = "enum class" if kind == "enum" else "data class"
            lines.append(f"{keyword} {name} {{}}")
        elif language == "swift":
            keyword = "enum" if kind == "enum" else "struct"
            lines.append(f"public {keyword} {name} {{}}")
        else:
            raise ValueError(f"unsupported fixture language: {language}")
    return "\n".join(lines)


def kotlin_surface(
    methods: Iterable[str],
    throwing_methods: Collection[str],
    type_declarations: Iterable[tuple[str, str]] = (),
) -> str:
    body = "\n".join(
        (
            "    @Throws(FfiException::class)\n"
            if method in throwing_methods
            else ""
        )
        + f"    override fun `{method}`() {{}}"
        for method in methods
    )
    declarations = _type_declarations(type_declarations, "kotlin")
    return f"{declarations}\ninterface MmpNodeInterface {{\n{body}\n}}\n"


def swift_surface(
    methods: Iterable[str],
    throwing_methods: Collection[str],
    type_declarations: Iterable[tuple[str, str]] = (),
) -> str:
    body = "\n".join(
        f"    func {method}(){' throws' if method in throwing_methods else ''}"
        for method in methods
    )
    declarations = _type_declarations(type_declarations, "swift")
    return f"{declarations}\nprotocol MmpNodeProtocol {{\n{body}\n}}\n"
