#!/usr/bin/env python3
from setuptools import find_packages, setup
import os
import platform


# TODO: this is duplicated with the up_aries module (needed to avoid install dependencies)
_EXECUTABLES = {
    ("Linux", "x86_64"): "bin/up-aries_linux_amd64",
    ("Linux", "aarch64"): "bin/up-aries_linux_arm64",
    ("Darwin", "x86_64"): "bin/up-aries_macos_amd64",
    ("Darwin", "aarch64"): "bin/up-aries_macos_arm64",
    ("Darwin", "arm64"): "bin/up-aries_macos_arm64",
    ("Windows", "AMD64"): "bin/up-aries_windows_amd64.exe",
    ("Windows", "aarch64"): "bin/up-aries_windows_arm64.exe",
}




def exists(executable):
    file = os.path.join(os.path.dirname(__file__), 'up_aries', executable)
    print(f"  {file}\t{os.path.exists(file)}")
    return os.path.exists(file) and os.path.isfile(file)


def check_self_executable():
    """Locates the Aries executable to use for the current platform."""
    try:
        filename = _EXECUTABLES[(platform.system(), platform.machine())]
    except KeyError:
        raise OSError(f"No executable for this platform: {platform.system()} / {platform.machine()}")
    if not exists(filename):
        raise FileNotFoundError(f"Could not locate executable: {filename}")


binaries = set(_EXECUTABLES.values())
print("Looking for installable binaries:")
binaries = list(filter(lambda f: exists(f), binaries))
check_self_executable()

setup(
    name="up_aries",
    version="0.0.2.dev0",
    description="Aries is a project aimed at exploring constraint-based techniques for automated planning and scheduling. \
        It relies on an original implementation of constraint solver with optional variables and clause learning to which \
        various automated planning problems can be submitted.",
    author="Arthur Bit-Monnot",
    author_email="abitmonnot@laas.fr",
    install_requires=["unified_planning", "grpcio", "grpcio-tools", "pytest"],
    packages=find_packages(include=["up_aries", "up_aries.*"]),
    package_data={"up_aries": ["bin/*"]},
    include_package_data=True,
    url="https://github.com/plaans/aries",
    license="MIT",
)
