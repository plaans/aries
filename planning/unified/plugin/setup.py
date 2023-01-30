#!/usr/bin/env python3
from setuptools import setup
import os
import up_aries


def exists(executable):
    file = os.path.join(os.path.dirname(__file__), executable)
    print(f"  {file}\t{os.path.exists(file)}")
    return os.path.exists(file)


binaries = set(up_aries._EXECUTABLES.values())
print("Looking for installable binaries:")
binaries = list(filter(lambda f: exists(f), binaries))

try:
    up_aries._executable()
except Exception as e:
    raise FileNotFoundError("No executable for current platform. ", str(e))


setup(
    name="up_aries",
    version="0.0.2",
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
