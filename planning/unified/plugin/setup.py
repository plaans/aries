#!/usr/bin/env python3
from setuptools import setup
import os
import platform


# TODO: this is duplicated with the up_aries module (needed to avoid install dependencies)
_EXECUTABLES = {
    ("Linux", "x86_64"): "bin/up-aries_linux_amd64",
    ("Linux", "aarch64"): "bin/up-aries_linux_arm64",
    ("Darwin", "x86_64"): "bin/up-aries_macos_amd64",
    ("Darwin", "aarch64"): "bin/up-aries_macos_arm64",
    ("Darwin", "arm64"): "bin/up-aries_macos_arm64",
    ("Windows", "x86_64"): "bin/up-aries_windows_amd64.exe",
    ("Windows", "aarch64"): "bin/up-aries_windows_arm64.exe",
}


def _executable():
    """Locates the Aries executable to use for the current platform."""
    try:
        filename = _EXECUTABLES[(platform.system(), platform.machine())]
    except KeyError:
        raise OSError(f"No executable for this platform: {platform.system()} / {platform.machine()}")
    exe = os.path.join(os.path.dirname(__file__), filename)
    if not os.path.exists(exe):
        raise FileNotFoundError(f"Could not locate executable: {exe}")
    if not os.path.isfile(exe):
        raise FileNotFoundError(f"Not a file: {exe}")
    return exe


def exists(executable):
    file = os.path.join(os.path.dirname(__file__), executable)
    print(f"  {file}\t{os.path.exists(file)}")
    return os.path.exists(file)


binaries = set(_EXECUTABLES.values())
print("Looking for installable binaries:")
binaries = list(filter(lambda f: exists(f), binaries))

try:
    _executable()
except Exception as e:
    raise FileNotFoundError("No executable for current platform. ", str(e))


setup(
    name="up_aries",
    version="0.0.2.dev0",
    description="Aries is a project aimed at exploring constraint-based techniques for automated planning and scheduling. \
        It relies on an original implementation of constraint solver with optional variables and clause learning to which \
        various automated planning problems can be submitted.",
    author="Arthur Bit-Monnot",
    author_email="abitmonnot@laas.fr",
    install_requires=["unified_planning", "grpcio", "grpcio-tools", "pytest"],
    py_modules=['up_aries'],
    data_files=[("bin", binaries)],
    url="https://github.com/plaans/aries",
    license="MIT",
)
