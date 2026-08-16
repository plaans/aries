"""APE command runner with error reporting."""

import psutil
import subprocess
import tempfile
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


class RSSMemoryLimitExceeded(subprocess.CalledProcessError):
    """Raised when a subprocess exceeds its (resident) memory limit."""

    def __init__(
        self,
        returncode: int,
        cmd: list[str],
        output: Optional[str] = None,
        stderr: Optional[str] = None,
        memory_limit_mb: Optional[int] = None,
    ):
        super().__init__(returncode, cmd, output, stderr)
        self.memory_limit_mb = memory_limit_mb

def _rss_bytes_recursive(proc) -> int:
    total = proc.memory_info().rss
    for child in proc.children(recursive=True):
        try:
            total += child.memory_info().rss
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
    return total

def _kill_proc_tree(proc) -> None:
    try:
        children = proc.children(recursive=True)
    except psutil.NoSuchProcess:
        children = []
    for child in children:
        try:
            child.kill()
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
    try:
        proc.kill()
    except (psutil.NoSuchProcess, psutil.AccessDenied):
        pass


@dataclass
class ApeResult:
    """Result of an APE command execution."""

    returncode: int
    stdout: str
    stderr: str
    command: list[str]

    @property
    def success(self) -> bool:
        """Whether the command succeeded."""
        return self.returncode == 0


def find_problem_file(plan_file: Path, ape_path: str = "target/ci/ape") -> Path:
    """
    Find problem file for a plan using APE's find-problem command.

    Args:
        plan_file: Path to plan file
        ape_path: Path to APE executable

    Returns:
        Path to problem file
    """
    result = subprocess.run(
        [ape_path, "find-problem", str(plan_file)],
        capture_output=True,
        text=True,
        check=True,
    )
    return Path(result.stdout.strip())


def find_domain_file(problem_file: Path, ape_path: str = "target/ci/ape") -> Path:
    """
    Find domain file for a problem using APE's find-domain command.

    Args:
        problem_file: Path to problem file
        ape_path: Path to APE executable

    Returns:
        Path to domain file
    """
    result = subprocess.run(
        [ape_path, "find-domain", str(problem_file)],
        capture_output=True,
        text=True,
        check=True,
    )
    return Path(result.stdout.strip())


class ApeRunner:
    """
    Runner for APE commands with automatic compilation and error reporting.

    APE is automatically compiled on initialization using the ci profile.

    Example:
        >>> ape = ApeRunner()  # Builds target/ci/ape
        >>> result = ape.run("validate", "plan.txt", "--expected-objective", "42")
        >>> if result.success:
        ...     print("Validation passed!")
    """

    def __init__(self, profile: str = "ci", auto_build: bool = True):
        """
        Initialize APE runner and optionally build APE.

        Args:
            profile: Cargo build profile (default: "ci")
            auto_build: If True, build APE on initialization (default: True)
        """
        self.profile = profile
        self.ape_path = f"target/{profile}/ape"

        if auto_build:
            self.build()

    def build(self) -> None:
        """Build APE from source using cargo with the configured profile."""
        print(f"Building APE (profile: {self.profile})...")
        subprocess.run(
            ["cargo", "build", "--profile", self.profile, "--bin", "ape"], check=True
        )

    def run(
        self,
        *args: str,
        timeout: Optional[int] = None,
        memory_limit_mb: Optional[int] = None,
        check: bool = True,
        capture_output: bool = True,
        env: Optional[dict[str, str]] = None,
    ) -> ApeResult:
        """
        Run an APE command with the given arguments.

        Args:
            *args: Arguments to pass to APE (e.g., "validate", "plan.txt")
            timeout: Optional timeout in seconds
            memory_limit_mb: Optional memory limit in MB (resident, monitored via psutil)
            check: If True, raise exception on non-zero exit code
            capture_output: If True, capture stdout/stderr (default: True)
            env: If not None, specifies the environment variables with which to run APE.
                 If None, inherits the current process' environment.
                 To use the specified environment on top of the inherited one, `env` will need to be built on top of the `os.environ.copy()` dict.

        Returns:
            ApeResult with command output and status

        Raises:
            subprocess.CalledProcessError: If check=True and command fails
            subprocess.TimeoutExpired: If command times out
            RSSMemoryLimitExceeded: If command exceeds the memory limit

        Example:
            >>> ape = ApeRunner()
            >>> result = ape.run("find-problem", "plan.txt")
            >>> print(result.stdout.strip())
            /path/to/problem.pddl
        """
        cmd = [self.ape_path] + list(args)

        try:
            if memory_limit_mb is not None:
                completed = self._run_with_memory_limit(cmd, timeout, memory_limit_mb, env=env)
            else:
                completed = subprocess.run(
                    cmd,
                    capture_output=capture_output,
                    text=True,
                    timeout=timeout,
                    check=False,
                    env=env,
                )

            ape_result = ApeResult(
                returncode=completed.returncode,
                stdout=completed.stdout if capture_output else "",
                stderr=completed.stderr if capture_output else "",
                command=cmd,
            )

            if check and completed.returncode != 0:
                self._report_error(ape_result)
                raise subprocess.CalledProcessError(
                    completed.returncode, cmd, completed.stdout, completed.stderr
                )

            return ape_result

        except subprocess.TimeoutExpired as e:
            self._report_timeout(cmd, timeout)
            raise e

    def _run_with_memory_limit(
        self,
        cmd: list[str],
        timeout: Optional[int],
        memory_limit_mb: int,
        env: Optional[dict[str, str]] = None,
    ) -> subprocess.CompletedProcess:
        exceeded = threading.Event()

        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env
        )

        monitor = threading.Thread(
            target=self._monitor_memory,
            args=(proc.pid, memory_limit_mb, exceeded),
            daemon=True,
        )
        monitor.start()

        try:
            stdout, stderr = proc.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.communicate()
            raise
        finally:
            monitor.join(timeout=2)

        completed = subprocess.CompletedProcess(cmd, proc.returncode, stdout, stderr)

        if exceeded.is_set():
            self._report_memory_limit(completed, memory_limit_mb)
            raise RSSMemoryLimitExceeded(
                completed.returncode,
                cmd,
                completed.stdout,
                completed.stderr,
                memory_limit_mb,
            )

        return completed

    @staticmethod
    def _monitor_memory(
        proc_pid: int, limit_mb: int, exceeded: threading.Event, poll_interval: float = 0.01
    ) -> None:
        limit_bytes = limit_mb * 1024 * 1024
        try:
            proc = psutil.Process(proc_pid)
        except psutil.NoSuchProcess:
            return
        while True:
            try:
                rss = _rss_bytes_recursive(proc)
                if rss > limit_bytes:
                    print(f"\nMemory limit exceeded ({limit_mb} MB, RSS={rss // 1024 // 1024} MB)")
                    exceeded.set()
                    _kill_proc_tree(proc)
                    return
                time.sleep(poll_interval)
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                return

    @staticmethod
    def _report_memory_limit(result: subprocess.CompletedProcess, limit_mb: int) -> None:
        print(f"\n{'=' * 60}")
        print(f"APE COMMAND MEMORY LIMIT EXCEEDED ({limit_mb} MB)")
        print(f"{'=' * 60}")
        print(f"\nCommand: {' '.join(result.args)}")
        #print("\nStdout:")
        #print(result.stdout if result.stdout else "(empty)")
        #print("\nStderr:")
        #print(result.stderr if result.stderr else "(empty)")
        print(f"\nSignal: {-result.returncode}")
        print("\nTo reproduce:")
        print(f"  systemd-run --user --scope -p MemoryMax=$(({limit_mb} * 1024 * 1024)) -- {' '.join(result.args)}")

    @staticmethod
    def _report_timeout(cmd: list[str], timeout: Optional[int]) -> None:
        print(f"\n{'=' * 60}")
        print(f"APE COMMAND TIMEOUT after {timeout}s")
        print(f"{'=' * 60}")
        print(f"\nCommand: {' '.join(cmd)}")
        print("\nTo reproduce:")
        print(f"  timeout {timeout}s {' '.join(cmd)}")

    @staticmethod
    def _report_error(result: ApeResult):
        """Report detailed error information."""
        print(f"\n{'=' * 60}")
        print("APE COMMAND FAILED")
        print(f"{'=' * 60}")
        print(f"\nCommand: {' '.join(result.command)}")
        print("\nStdout:")
        print(result.stdout if result.stdout else "(empty)")
        print("\nStderr:")
        print(result.stderr if result.stderr else "(empty)")
        print(f"\nReturn code: {result.returncode}")
        print("\nTo reproduce:")
        print(f"  {' '.join(result.command)}")

    def find_problem(self, plan_file: Path) -> Path:
        """
        Find problem file for a plan using 'ape find-problem'.

        Args:
            plan_file: Path to plan file

        Returns:
            Path to problem file
        """
        result = self.run("find-problem", str(plan_file))
        return Path(result.stdout.strip())

    def find_domain(self, problem_file: Path) -> Path:
        """
        Find domain file for a problem using 'ape find-domain'.

        Args:
            problem_file: Path to problem file

        Returns:
            Path to domain file
        """
        result = self.run("find-domain", str(problem_file))
        return Path(result.stdout.strip())

    def validate(
        self, plan_file: Path, expected_objective: str, timeout: int = 5
    ) -> bool:
        """
        Validate a plan with APE.

        Args:
            plan_file: Path to plan file
            expected_objective: Expected plan quality/objective value
            timeout: Timeout in seconds (default: 5)

        Returns:
            True if validation succeeds, False otherwise
        """
        try:
            self.run(
                "validate",
                str(plan_file),
                "--expected-objective",
                expected_objective,
                timeout=timeout,
            )
            return True
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired, RSSMemoryLimitExceeded):
            return False

    def optimize_plan(
        self,
        plan_file: Path,
        relaxations: list[str],
        timeout: int = 5,
        validate_result: bool = True,
        val_path: str = "planning/ext/val-pddl",
    ) -> bool:
        """
        Run APE plan optimizer and optionally validate the result.

        Args:
            plan_file: Path to plan file
            relaxations: List of relaxations to apply (e.g., ["action-presence", "start-time"])
            timeout: Timeout in seconds (default: 5)
            validate_result: If True, write optimized plan to temp file and validate with VAL
            val_path: Path to VAL validator (used if validate_result=True)

        Returns:
            True if optimization succeeds (and validation passes if enabled), False otherwise
        """
        args = ["optimize-plan", str(plan_file)]
        for relaxation in relaxations:
            args.extend(["-r", relaxation])

        if validate_result:
            # Create temporary file for optimized plan
            with tempfile.NamedTemporaryFile(
                mode="w", suffix=".plan", delete=False
            ) as tmp:
                tmp_plan = Path(tmp.name)

            try:
                # Add -w flag to write optimized plan
                args.extend(["-w", str(tmp_plan)])

                # Run optimizer
                self.run(*args, timeout=timeout)

                # Find domain and problem files for validation
                problem_file = self.find_problem(plan_file)
                domain_file = self.find_domain(problem_file)

                # Validate optimized plan with VAL
                from .validation import validate_plan

                result = validate_plan(domain_file, problem_file, tmp_plan, val_path)

                if not result.is_valid:
                    print(f"\n{'=' * 60}")
                    print("OPTIMIZATION FAILED: Optimized plan is invalid")
                    print(f"{'=' * 60}")
                    print(f"\nOriginal plan: {plan_file}")
                    print(f"Optimized plan: {tmp_plan}")
                    print("\nVAL output:")
                    print(result.stdout)
                    print("\nTo reproduce:")
                    print(f"  {' '.join(args)}")
                    print(f"  {val_path} {domain_file} {problem_file} {tmp_plan}")
                    return False

                return True

            except (subprocess.CalledProcessError, subprocess.TimeoutExpired, RSSMemoryLimitExceeded):
                return False
            finally:
                # Clean up temporary file
                if tmp_plan.exists():
                    tmp_plan.unlink()
        else:
            # Run without validation
            try:
                self.run(*args, timeout=timeout)
                return True
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired, RSSMemoryLimitExceeded):
                return False
