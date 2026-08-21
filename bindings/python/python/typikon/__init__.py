"""Safe Python facade for the Typikon PyO3 extension."""

from __future__ import annotations

from importlib import import_module
from typing import Any

_native = import_module("typikon_python")


class BorrowedPacket:
    """Validated packet owner with a packet-backed memoryview.

    Typed Python strings, lists, and dictionaries are intentionally decoded as
    owned Python objects. The borrowed API exposes bytes and packet ownership,
    where the lifetime contract is explicit and safe.
    """

    __slots__ = ("_wire", "type_name")

    def __init__(self, wire: bytes, type_name: str) -> None:
        self._wire = memoryview(wire)
        self.type_name = type_name

    @property
    def wire(self) -> memoryview:
        return self._wire


def _function_name(type_name: str) -> str:
    return "".join(
        ("_" if index and char.isupper() else "") + char.lower()
        for index, char in enumerate(type_name)
    )


def borrowed_packet(type_name: str, wire: bytes) -> BorrowedPacket:
    """Validate *wire* for a schema type and retain its backing owner."""

    validator = getattr(_native, f"validate_borrowed_{_function_name(type_name)}")
    validator(wire)
    return BorrowedPacket(wire, type_name)


def __getattr__(name: str) -> Any:
    """Expose generated native functions through the package facade."""

    return getattr(_native, name)


__all__ = ["BorrowedPacket", "borrowed_packet"]
