#!/usr/bin/env python3
"""Detector for different architectures."""
import platform

EXECUTABLES = {
    ("Linux", "x86_64"): "bins/aries_linux_x86_64",
    ("Linux", "aarch64"): "bins/aries_linux_aarch64",
    ("Darwin", "x86_64"): "bins/aries_macos_x86_64",
    ("Darwin", "aarch64"): "bins/aries_macos_aarch64",
    ("Darwin", "arm64"): "bins/aries_macos_aarch64",
    # ("Windows", "x86_64"): "aries_windows_x86_64.exe",
    # ("Windows", "aarch64"): "aries_windows_aarch64.exe",
    # ("Windows", "x86"): "aries_windows_x86.exe",
    # ("Windows", "aarch32"): "aries_windows_aarch32.exe",
}


class Executor:
    def __call__(self):
        try:
            return EXECUTABLES[(self._get_os(), self._get_architecture())]
        except KeyError:
            raise OSError("No executable for this platform")

    def _get_architecture(self) -> str:
        return platform.machine()

    def _get_os(self) -> str:
        return platform.system()
